//! Single-threaded async executor for tur — dependency-free.
//!
//! The engine drives all async work from its main thread (the JNI thread on
//! Android, the winit thread on desktop, the rAF callback on wasm). This
//! executor is a small, self-contained cooperative scheduler: a ready queue
//! plus a hand-rolled [`RawWaker`] over `Rc`. The engine calls
//! [`Executor::tick`] once per frame inside `flush`, which polls every ready
//! task exactly once.
//!
//! ## Why dependency-free
//!
//! The engine is *embeddable*: it must not bring its own async runtime. A
//! network backend that needs a real reactor (e.g. native `reqwest`, whose
//! I/O + DNS need a driven tokio reactor) hosts that reactor itself — outside
//! the engine — and marshals results back through a channel whose
//! `Receiver` future is reactor-agnostic and polls cleanly under this
//! executor. See `tur-net-native` for the canonical pattern: the user builds
//! and owns a tokio runtime, passes its `Handle` to `NativeHttp`, which
//! `spawn`s each request and bridges the result back via
//! `tokio::sync::oneshot`.
//!
//! The engine-managed timer queue ([`TimerQueue`]) + [`Sleep`] stay
//! hand-rolled so frame scheduling (`next_timer_deadline` → `NextFrame::After`)
//! is uniform across platforms; `Sleep` is reactor-agnostic (it stores
//! `cx.waker()`, which is the hand-rolled waker here).
//!
//! ## Threading
//!
//! Everything runs on the calling thread. Tasks are `!Send` (engine futures
//! capture `Rc`); wakers are `!Send` (built on `Rc`). The only cross-thread
//! traffic in the whole engine is HTTP I/O spawned onto the user's tokio
//! workers, whose results marshal back via `oneshot`/`mpsc` receivers polled
//! here, on the main thread.
//!
//! Public surface (preserved from the previous tokio-backed executor):
//! [`Executor::new`] / [`Executor::spawn`] / [`Executor::spawn_detached`] /
//! [`Executor::spawn_task`] / [`Executor::sleep`] /
//! [`Executor::next_timer_deadline`] / [`Executor::now`] / [`Executor::tick`]
//! / [`Executor::has_pending`], plus the [`Task`] cancellation handle and the
//! [`Sleep`] / [`Clock`] / [`TimerQueue`] types. The engine wrapper in
//! `tur_engine::core::async_::AsyncExecutor` is unchanged.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::time::Duration;

mod sleep;

pub use sleep::Sleep;

/// Wall-clock time source for [`Executor::sleep`] and timer management.
///
/// Implementations must use a wasm-compatible source (e.g.
/// `js_sys::Date::now()`) — `std::time::Instant::now()` panics on
/// `wasm32-unknown-unknown`. The engine provides an adapter from its own
/// `Clock` (boa's trait) so backends only implement one time source.
pub trait Clock: 'static {
    /// Wall-clock now, as a `Duration` since the Unix epoch.
    fn now(&self) -> Duration;
}

/// Timer queue: absolute deadline → wakers waiting for that deadline. Drained
/// by [`Executor::tick`] (expired entries are woken) and read by
/// [`Executor::next_timer_deadline`] for frame-loop scheduling. Stored wakers
/// are whatever the polling executor handed `Sleep` (the hand-rolled waker
/// here).
pub(crate) type TimerQueue = Rc<RefCell<BTreeMap<Duration, Vec<Waker>>>>;

type BoxFuture = Pin<Box<dyn Future<Output = ()>>>;
type ReadyQueue = Rc<RefCell<VecDeque<Rc<TaskCell>>>>;

/// Cancellation handle for a spawned task. Dropping it requests cancellation:
/// the wrapped future sees the shared `terminated` flag on its next poll and
/// self-completes. This is a *soft* cancel (effective on the next tick), not
/// an immediate abort — deliberately, because re-entering the ready queue
/// from a `Drop` that may run inside another thread-local's destructor during
/// teardown would be unsafe. Soft-cancel touches only the shared `Cell`,
/// which is sound anywhere. Created by [`Executor::spawn_task`].
/// For fire-and-forget tasks that don't need cancellation, use
/// [`Executor::spawn`] instead.
pub struct Task {
    /// Shared with the [`Counted`] wrapper. Set by [`Task::drop`] (cancel) or
    /// by `Counted` on natural completion. [`Task::is_alive`] reads it.
    terminated: Rc<Cell<bool>>,
}

impl Task {
    /// Returns `true` if the task is still alive (not cancelled and not
    /// completed).
    pub fn is_alive(&self) -> bool {
        !self.terminated.get()
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        // Soft-cancel: just flag the wrapper. Safe even inside teardown.
        self.terminated.set(true);
    }
}

/// A schedulable task — an `Rc`-shared cell holding the future + bookkeeping.
/// The waker re-enqueues clones of this `Rc` into the executor's ready queue.
struct TaskCell {
    inner: RefCell<TaskInner>,
}

struct TaskInner {
    /// `None` once the task has completed or been cancelled. Polled futures
    /// are taken out into a local during [`Executor::tick`] so the waker can
    /// borrow `inner` without a re-borrow conflict.
    fut: Option<BoxFuture>,
    /// `true` while this task is already in the ready queue. The waker checks
    /// this to avoid flooding the queue with duplicate entries on repeated
    /// wake-ups.
    queued: bool,
    /// The owning executor's ready queue — needed by the waker to re-enqueue.
    ready: ReadyQueue,
}

impl Drop for TaskInner {
    fn drop(&mut self) {
        // Drop any lingering future explicitly so `Counted::drop` runs and the
        // live-task counter stays accurate if the executor is dropped while a
        // task is still pending.
        self.fut.take();
    }
}

/// Wrapper applied to every spawned future. Sets the shared `tick_polled` flag
/// on each poll (so [`Executor::tick`] can report progress), maintains the
/// shared `live` count (decremented on drop — covers both completion and the
/// task being dropped with the executor), and self-completes if the soft-cancel
/// flag is set.
///
/// `Counted: Unpin` (all fields are `Unpin`), so `Pin<&mut Counted>` can freely
/// project to the inner `BoxFuture` — no `unsafe`.
struct Counted {
    fut: BoxFuture,
    live: Rc<Cell<usize>>,
    tick_polled: Rc<Cell<bool>>,
    /// `Some` for `spawn_task` tasks (shared with the returned [`Task`]);
    /// `None` for fire-and-forget `spawn` tasks (no cancellation).
    terminated: Option<Rc<Cell<bool>>>,
}

impl Future for Counted {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        // Soft-cancel: if the Task was dropped, terminate on the next poll.
        if let Some(terminated) = &this.terminated
            && terminated.get()
        {
            return Poll::Ready(());
        }
        this.tick_polled.set(true);
        match this.fut.as_mut().poll(cx) {
            Poll::Ready(()) => {
                if let Some(terminated) = &this.terminated {
                    terminated.set(true);
                }
                Poll::Ready(())
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.live.set(self.live.get().saturating_sub(1));
    }
}

/// Engine-owned, engine-driven single-threaded executor — a ready queue + a
/// hand-rolled waker.
///
/// Held as `Rc<Executor>` (via the engine's `AsyncExecutor` wrapper) and
/// driven from the main thread: [`Executor::tick`] polls every ready task
/// exactly once there. Tasks are `!Send` (engine futures capture `Rc`), so
/// they live on the calling thread.
pub struct Executor {
    ready: ReadyQueue,
    timers: TimerQueue,
    clock: Rc<dyn Clock>,
    /// Live (uncompleted) spawned-task count. Drives [`Executor::has_pending`],
    /// which the frame loop uses to keep pumping frames while an async task is
    /// in flight (e.g. an HTTP request).
    live: Rc<Cell<usize>>,
    /// Set to `true` by the [`Counted`] wrapper whenever a task is polled.
    /// [`Executor::tick`] resets it before driving and reads it after, so its
    /// return value reflects "did any task make progress" — which the flush
    /// fixed-point loop relies on for its quiescence test.
    tick_polled: Rc<Cell<bool>>,
}

impl Executor {
    /// Create an executor with a wall-clock time source for `sleep`/`tick`
    /// timer management.
    pub fn new(clock: Rc<dyn Clock>) -> Self {
        Executor {
            ready: Rc::new(RefCell::new(VecDeque::new())),
            timers: Rc::new(RefCell::new(BTreeMap::new())),
            clock,
            live: Rc::new(Cell::new(0)),
            tick_polled: Rc::new(Cell::new(false)),
        }
    }

    /// Spawn a `!Send` future. The future lives until it returns `Ready` or
    /// the executor is dropped — there is no explicit cancellation API
    /// (matches the fire-and-forget pattern used by clipboard/http bridge fns).
    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + 'static,
    {
        self.spawn_counted(Box::pin(fut), None);
    }

    /// Alias for [`Self::spawn`]; spelling preserved for callers that
    /// originally came from an executor API where spawn returned a Task.
    pub fn spawn_detached<F>(&self, fut: F)
    where
        F: Future<Output = ()> + 'static,
    {
        self.spawn(fut);
    }

    /// Spawn a `!Send` future and return a cancellable [`Task`]. Dropping the
    /// task requests a soft cancel (effective on the next tick).
    pub fn spawn_task<F>(&self, fut: F) -> Task
    where
        F: Future<Output = ()> + 'static,
    {
        let terminated = Rc::new(Cell::new(false));
        self.spawn_counted(Box::pin(fut), Some(terminated.clone()));
        Task { terminated }
    }

    /// Shared spawn path: wrap the future in [`Counted`] (tracks live-task
    /// count + tick-progress flag + soft-cancel), build its [`TaskCell`], and
    /// enqueue it.
    fn spawn_counted(&self, fut: BoxFuture, terminated: Option<Rc<Cell<bool>>>) {
        self.live.set(self.live.get() + 1);
        let cell = Rc::new(TaskCell {
            inner: RefCell::new(TaskInner {
                fut: Some(Box::pin(Counted {
                    fut,
                    live: self.live.clone(),
                    tick_polled: self.tick_polled.clone(),
                    terminated: terminated.clone(),
                })),
                queued: true,
                ready: self.ready.clone(),
            }),
        });
        self.ready.borrow_mut().push_back(cell);
    }

    /// Create a [`Sleep`] future that completes after `duration`.
    ///
    /// The future registers its waker in the executor's timer queue on first
    /// poll. The engine frame loop reads [`Self::next_timer_deadline`] to
    /// schedule a precise wake-up via `NextFrame::After(d)` instead of
    /// busy-polling at vsync.
    pub fn sleep(&self, duration: Duration) -> Sleep {
        let deadline = self.clock.now() + duration;
        Sleep {
            deadline,
            timers: self.timers.clone(),
            clock: Rc::downgrade(&self.clock),
            registered: false,
        }
    }

    /// Returns the earliest pending timer deadline, if any. Used by the engine
    /// frame loop to schedule `NextFrame::After(d)` for timer-driven async
    /// tasks (e.g. caret blink).
    pub fn next_timer_deadline(&self) -> Option<Duration> {
        self.timers.borrow().keys().next().copied()
    }

    /// Returns the current wall-clock time.
    pub fn now(&self) -> Duration {
        self.clock.now()
    }

    /// Drive all ready tasks one poll step. Returns `true` if any task was
    /// polled.
    ///
    /// First, expired timer entries are woken (their wakers re-enqueue the
    /// task in the ready queue). Then every task currently in the ready queue
    /// is polled exactly once with its waker. A task that returns `Pending`
    /// stays out of the queue until its waker is invoked (which re-enqueues
    /// it). Tasks enqueued by wakes during this loop are picked up in the same
    /// loop (we keep draining until the queue is empty) — matching the
    /// fixed-point behaviour the flush loop relies on. The [`Counted`]
    /// wrapper sets `tick_polled` if any task was polled; that flag is this
    /// function's return value.
    pub fn tick(&self) -> bool {
        // Wake expired timers (their stored wakers fire here, on the main
        // thread — same-thread for the engine's single-threaded model).
        let now = self.clock.now();
        let expired: Vec<Vec<Waker>> = {
            let mut timers = self.timers.borrow_mut();
            let keys: Vec<Duration> = timers.keys().take_while(|&&k| k <= now).copied().collect();
            keys.into_iter().filter_map(|k| timers.remove(&k)).collect()
        };
        for wakers in expired {
            for waker in wakers {
                waker.wake();
            }
        }

        // Reset the progress flag, drive one cooperative step, read it back.
        self.tick_polled.set(false);
        loop {
            let cell = match self.ready.borrow_mut().pop_front() {
                Some(c) => c,
                None => break,
            };
            // About to poll: clear `queued` (so wakes during poll re-enqueue)
            // and skip stale entries (completed since they were enqueued).
            {
                let mut inner = cell.inner.borrow_mut();
                if inner.fut.is_none() {
                    inner.queued = false;
                    continue;
                }
                inner.queued = false;
            }
            // Take the future out so the waker path can borrow `inner`
            // without a re-borrow panic during poll.
            let mut fut = cell
                .inner
                .borrow_mut()
                .fut
                .take()
                .expect("fut presence checked above");
            let waker = task_waker(cell.clone());
            let mut cx = Context::from_waker(&waker);
            let result = fut.as_mut().poll(&mut cx);
            match result {
                Poll::Ready(()) => {
                    // Dropping `fut` runs `Counted::drop` (live-- + terminated).
                    drop(fut);
                    // `inner.fut` stays None; the task is finished.
                }
                Poll::Pending => {
                    // Restore. If wake fired during poll, the cell is already
                    // enqueued (queued=true); we don't push again.
                    cell.inner.borrow_mut().fut = Some(fut);
                }
            }
        }
        self.tick_polled.get()
    }

    /// True if there is pending work (any live spawned task). Used by the frame
    /// loop to decide `NextFrame::Vsync` (continuous) vs `Idle`. A task
    /// awaiting a `Sleep` timer is still live — it just isn't immediately-ready
    /// work, so the schedule decision below combines this with
    /// `next_timer_deadline`.
    pub fn has_pending(&self) -> bool {
        self.live.get() > 0
    }
}

// ---------------------------------------------------------------------------
// Waker — a sound RawWaker over `Rc<TaskCell>`
// ---------------------------------------------------------------------------
//
// `std::task::Wake` requires `Arc<Self>: Send + Sync + 'static`, which a
// `!Send` single-threaded executor cannot satisfy (engine futures capture
// `Rc`). We therefore build the `Waker` directly via `Waker::from_raw` with a
// small vtable whose only `unsafe` is the well-known `Rc` refcount dance. The
// `Rc<TaskCell>` "owned" by a waker is represented as a raw pointer obtained
// from `Rc::into_raw`; clone/wake/wake_by_ref/drop each adjust the refcount
// accordingly. This is the same pattern tokio's `LocalSet` and the `async-task`
// crate use internally.

/// Build the [`Waker`] for a task. The waker holds one strong reference to the
/// `Rc<TaskCell>` (via `Rc::into_raw`); cloning/waking/dropping the waker
/// correctly manages that refcount via the vtable below.
fn task_waker(task: Rc<TaskCell>) -> Waker {
    let ptr = Rc::into_raw(task) as *const ();
    // SAFETY: `ptr` is a valid `Rc<TaskCell>` raw pointer (produced by
    // `Rc::into_raw`), and the vtable functions below correctly manage the
    // `Rc` refcount for clone/wake/wake_by_ref/drop.
    unsafe { Waker::from_raw(RawWaker::new(ptr, &TASK_WAKER_VTABLE)) }
}

/// Clone: increment the refcount and return a new `RawWaker`.
unsafe fn clone_task_waker(ptr: *const ()) -> RawWaker {
    // SAFETY: `ptr` came from `Rc::into_raw` in `task_waker` (or a prior
    // clone); adopting it as an `Rc` here borrows the existing refcount
    // without changing it.
    let arc = unsafe { Rc::from_raw(ptr as *const TaskCell) };
    let cloned = arc.clone();
    std::mem::forget(arc); // don't touch the original's refcount
    RawWaker::new(Rc::into_raw(cloned) as *const (), &TASK_WAKER_VTABLE)
}

/// Wake (consuming): take ownership of one refcount and enqueue.
unsafe fn wake_task_waker(ptr: *const ()) {
    // SAFETY: `ptr` is valid; this function consumes the refcount the
    // `RawWaker` held (the `Rc` drops at end-of-scope).
    let arc = unsafe { Rc::from_raw(ptr as *const TaskCell) };
    wake_inner(&arc);
}

/// Wake by ref: enqueue without consuming the refcount.
unsafe fn wake_task_waker_by_ref(ptr: *const ()) {
    // SAFETY: `ptr` is valid; forgetting the temporary `Rc` borrows the
    // refcount without releasing it.
    let arc = unsafe { Rc::from_raw(ptr as *const TaskCell) };
    wake_inner(&arc);
    std::mem::forget(arc);
}

/// Drop (consuming): release one refcount.
unsafe fn drop_task_waker(ptr: *const ()) {
    // SAFETY: `ptr` is valid; dropping the adopted `Rc` decrements the
    // refcount, freeing the `TaskCell` when it reaches zero.
    drop(unsafe { Rc::from_raw(ptr as *const TaskCell) });
}

const TASK_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    clone_task_waker,
    wake_task_waker,
    wake_task_waker_by_ref,
    drop_task_waker,
);

/// Shared wake path. Enqueues the task into the ready queue (idempotent via
/// the `queued` flag) so the next [`Executor::tick`] polls it.
fn wake_inner(task: &Rc<TaskCell>) {
    let mut inner = task.inner.borrow_mut();
    if inner.queued {
        return;
    }
    inner.queued = true;
    let ready = inner.ready.clone();
    drop(inner);
    ready.borrow_mut().push_back(task.clone());
}
