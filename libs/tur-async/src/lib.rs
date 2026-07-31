//! Single-threaded async executor for tur, backed by Tokio.
//!
//! The engine drives all async work from its main thread (the JNI thread on
//! Android, the winit thread on desktop, the rAF callback on wasm). This
//! executor wraps a Tokio **current-thread** runtime + [`LocalSet`] and is
//! driven cooperatively: the engine calls [`Executor::tick`] once per frame
//! inside `flush`, which runs `LocalSet::block_on(&rt, yield_now())` — one
//! scheduler + reactor pass over the calling (main) thread.
//!
//! Why Tokio everywhere (wasm included): the previous hand-rolled `Rc`-waker
//! executor duplicated a lot of `unsafe` `RawWaker` machinery and could not
//! host futures whose wake-ups originate in a real reactor (e.g. native
//! `reqwest`, whose I/O + DNS need a driven reactor). Tokio gives us sound,
//! `Send` wakers for free; on wasm, `reqwest-wasm`'s browser wake-ups compose
//! with Tokio wakers (both are just `std::task::Waker` invoked on the main
//! thread — verified by a dedicated spike).
//!
//! The engine-managed timer queue ([`TimerQueue`]) + [`Sleep`] stay
//! hand-rolled so frame scheduling (`next_timer_deadline` → `NextFrame::After`)
//! is uniform across platforms; `Sleep` is reactor-agnostic (it stores
//! `cx.waker()`, which is a Tokio waker here).
//!
//! Public surface (preserved from the old executor):
//! [`Executor::new`] / [`Executor::spawn`] / [`Executor::spawn_detached`] /
//! [`Executor::spawn_task`] / [`Executor::sleep`] /
//! [`Executor::next_timer_deadline`] / [`Executor::now`] / [`Executor::tick`]
//! / [`Executor::has_pending`], plus the [`Task`] cancellation handle and the
//! [`Sleep`] / [`Clock`] / [`TimerQueue`] types. The engine wrapper in
//! `tur_engine::core::async_::AsyncExecutor` is unchanged.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::runtime::{Builder, Runtime};
use tokio::task::LocalSet;

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
/// are whatever the polling executor handed `Sleep` (a Tokio waker here).
pub(crate) type TimerQueue = Rc<std::cell::RefCell<BTreeMap<Duration, Vec<std::task::Waker>>>>;

type BoxFuture = Pin<Box<dyn Future<Output = ()>>>;

/// Cancellation handle for a spawned task. Dropping it requests cancellation:
/// the wrapped future sees the shared `terminated` flag on its next poll and
/// self-completes. This is a *soft* cancel (effective on the next tick), not
/// an immediate `AbortHandle::abort` — deliberately, because `abort` would
/// call into tokio's `LocalSet::schedule`, which accesses a thread-local that
/// is unsafe to touch during teardown (e.g. when an element holding a `Task`
/// is dropped inside boa's GC thread-local destructor). Soft-cancel avoids
/// any thread-local access in `Drop`. Created by [`Executor::spawn_task`].
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
        // Soft-cancel: just flag the wrapper. No tokio thread-local access —
        // safe even when this Drop runs inside another thread-local's
        // destructor during teardown.
        self.terminated.set(true);
    }
}

/// Engine-owned, engine-driven single-threaded executor backed by a Tokio
/// current-thread runtime + [`LocalSet`].
///
/// Held as `Rc<Executor>` (via the engine's `AsyncExecutor` wrapper) and
/// driven from the main thread: [`Executor::tick`] runs one cooperative
/// scheduler + reactor pass there. Tasks are `!Send` (engine futures capture
/// `Rc`), so they live on the `LocalSet` of the calling thread.
pub struct Executor {
    rt: Runtime,
    local: LocalSet,
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
    /// timer management. Builds a current-thread Tokio runtime (with I/O +
    /// time drivers on native; scheduler-only on wasm — `reqwest-wasm` uses
    /// the browser for I/O, and engine timers stay engine-managed).
    pub fn new(clock: Rc<dyn Clock>) -> Self {
        Executor {
            rt: build_runtime(),
            local: LocalSet::new(),
            timers: Rc::new(std::cell::RefCell::new(BTreeMap::new())),
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
        let _handle = self.spawn_counted(Box::pin(fut), Some(terminated.clone()));
        // Detach the JoinHandle; cancellation is via the shared flag, not abort.
        Task { terminated }
    }

    /// Shared spawn path: wrap the future in [`Counted`] (tracks live-task
    /// count + tick-progress flag + soft-cancel), enter the `LocalSet` so
    /// `spawn_local` targets it, and spawn. Returns the `JoinHandle` (unused
    /// by callers — `spawn_task` cancels via the shared flag, not abort).
    fn spawn_counted(
        &self,
        fut: BoxFuture,
        terminated: Option<Rc<Cell<bool>>>,
    ) -> tokio::task::JoinHandle<()> {
        self.live.set(self.live.get() + 1);
        let counted = Counted {
            fut,
            live: self.live.clone(),
            tick_polled: self.tick_polled.clone(),
            terminated,
        };
        // `spawn_local` requires the LocalSet's context; `enter()` provides it
        // without us having to be inside `block_on` (the engine spawns from
        // synchronous boa callbacks between ticks).
        let _enter = self.local.enter();
        self.local.spawn_local(counted)
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
    /// First, expired timer entries are woken (their wakers — Tokio wakers —
    /// re-enqueue the task in the runtime). Then `LocalSet::block_on(&rt,
    /// yield_now())` runs one scheduler + reactor pass on the calling (main)
    /// thread, polling every ready task once. The [`Counted`] wrapper sets
    /// `tick_polled` if any task was polled; that flag is this function's
    /// return value, which the flush fixed-point loop uses for quiescence
    /// detection.
    pub fn tick(&self) -> bool {
        // Wake expired timers (their stored wakers fire here, on the main
        // thread — same-thread for the engine's single-threaded model).
        let now = self.clock.now();
        let expired: Vec<Vec<std::task::Waker>> = {
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
        self.local.block_on(&self.rt, async {
            tokio::task::yield_now().await;
        });
        self.tick_polled.get()
    }

    /// True if there is pending work (any live spawned task). Used by the frame
    /// loop to decide `NextFrame::Vsync` (continuous) vs `Idle`. A task
    /// awaiting a `Sleep` timer is still live — it just isn't immediately-ready
    /// work, so the schedule decision below combines this with
    /// `next_timer_delay`.
    pub fn has_pending(&self) -> bool {
        self.live.get() > 0
    }
}

/// Wrapper applied to every spawned future. Sets the shared `tick_polled` flag
/// on each poll (so [`Executor::tick`] can report progress) and maintains the
/// shared `live` count (decremented on drop — covers both completion and the
/// task being dropped with the runtime). `terminated` is the soft-cancel
/// channel shared with [`Task`]: set externally (Task drop = cancel) or
/// internally (natural completion) — the wrapper checks it on each poll and
/// self-completes if set.
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

/// Build the current-thread Tokio runtime.
///
/// - **Native:** enable I/O + time drivers (`enable_all`) so `reqwest`/hyper's
///   sockets + timeouts + `spawn_blocking` DNS work. Workers aren't used (it's
///   current-thread); the blocking pool handles `spawn_blocking`.
/// - **wasm:** scheduler-only (`net` is unavailable on `wasm32`; `reqwest-wasm`
///   uses the browser for I/O, and engine timers are engine-managed so no
///   Tokio time driver is needed either).
#[cfg(not(target_family = "wasm"))]
fn build_runtime() -> Runtime {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
}

#[cfg(target_family = "wasm")]
fn build_runtime() -> Runtime {
    Builder::new_current_thread()
        .build()
        .expect("failed to build tokio runtime")
}
