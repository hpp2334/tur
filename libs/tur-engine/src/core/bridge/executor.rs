use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context as TaskContext, Poll, RawWaker, RawWakerVTable, Waker};

use boa_engine::context::time::{JsDuration, JsInstant};
use boa_engine::job::{
    GenericJob, IntervalJob, Job, JobExecutor, NativeAsyncJob, PromiseJob, TimeoutJob,
};
use boa_engine::Context;
use boa_engine::JsResult;

enum ClockJob {
    Timeout(TimeoutJob),
    Interval(IntervalJob),
}

impl ClockJob {
    fn cancelled(&self) -> bool {
        match self {
            ClockJob::Timeout(t) => t.cancelled(),
            ClockJob::Interval(i) => i.cancelled(),
        }
    }
}

#[derive(Default)]
pub struct TurJobExecutor {
    promise_jobs: RefCell<VecDeque<PromiseJob>>,
    generic_jobs: RefCell<VecDeque<GenericJob>>,
    async_jobs: RefCell<VecDeque<NativeAsyncJob>>,
    clock_jobs: RefCell<BTreeMap<JsInstant, Vec<ClockJob>>>,
}

impl TurJobExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    /// True if there are pending `setTimeout`/`setInterval` jobs not yet due.
    /// Used by the frame scheduler to keep the loop advancing the clock (so
    /// pending timers eventually fire) instead of going idle.
    pub fn has_pending_clock_jobs(&self) -> bool {
        !self.clock_jobs.borrow().is_empty()
    }

    /// Time from `now` until the soonest pending `setTimeout`/`setInterval`
    /// job is due, or `None` if no clock job is pending. Lets the frame
    /// scheduler wake precisely at a timer's deadline (one frame) instead of
    /// polling at vsync while a long interval is outstanding.
    pub fn next_clock_job_delay(&self, now: JsInstant) -> Option<std::time::Duration> {
        let jobs = self.clock_jobs.borrow();
        let deadline = *jobs.keys().next()?;
        let delay: JsDuration = deadline - now;
        Some(delay.into())
    }

    pub fn drain(&self, context: &mut Context) -> JsResult<usize> {        let mut count = 0;

        let now = context.clock().now();
        let due = {
            let mut all = self.clock_jobs.borrow_mut();
            let keep = all.split_off(&now);
            let mut due = std::mem::replace(&mut *all, keep);
            if let Some(at_now) = all.remove(&now) {
                due.insert(now, at_now);
            }
            due
        };

        for (_instant, jobs) in due {
            for job in jobs {
                if job.cancelled() {
                    continue;
                }
                match job {
                    ClockJob::Timeout(t) => {
                        t.call(context)?;
                    }
                    ClockJob::Interval(i) => {
                        i.call(context)?;
                        let now = context.clock().now();
                        self.clock_jobs
                            .borrow_mut()
                            .entry(now + i.interval())
                            .or_default()
                            .push(ClockJob::Interval(i));
                    }
                }
            }
        }

        // Poll async jobs (module loading, finalization-registry cleanup) to
        // completion. These are cooperative single-threaded futures that
        // resolve on the first poll; their completion enqueues promise jobs
        // which the step below picks up (now or on the next `drain` call).
        let async_jobs: Vec<NativeAsyncJob> = self.async_jobs.borrow_mut().drain(..).collect();
        if !async_jobs.is_empty() {
            // `NativeAsyncJob::call` wants a `&RefCell<&mut Context>`.
            let context_cell = RefCell::new(&mut *context);
            for job in async_jobs {
                poll_async_job_to_completion(job, &context_cell)?;
                count += 1;
            }
        }

        let promise_jobs: Vec<_> = self.promise_jobs.borrow_mut().drain(..).collect();
        for job in promise_jobs {
            if let Err(e) = job.call(context) {
                tracing::error!("promise job error: {e}");
            }
            count += 1;
        }

        let generic_jobs: Vec<_> = self.generic_jobs.borrow_mut().drain(..).collect();
        for job in generic_jobs {
            if let Err(e) = job.call(context) {
                tracing::error!("generic job error: {e}");
            }
            count += 1;
        }

        Ok(count)
    }
}

impl JobExecutor for TurJobExecutor {
    fn enqueue_job(self: Rc<Self>, job: Job, context: &mut Context) {
        match job {
            Job::PromiseJob(p) => self.promise_jobs.borrow_mut().push_back(p),
            Job::GenericJob(g) => self.generic_jobs.borrow_mut().push_back(g),
            Job::TimeoutJob(t) => {
                let now = context.clock().now();
                self.clock_jobs
                    .borrow_mut()
                    .entry(now + t.timeout())
                    .or_default()
                    .push(ClockJob::Timeout(t));
            }
            Job::IntervalJob(i) => {
                let now = context.clock().now();
                self.clock_jobs
                    .borrow_mut()
                    .entry(now + i.interval())
                    .or_default()
                    .push(ClockJob::Interval(i));
            }
            Job::AsyncJob(a) => self.async_jobs.borrow_mut().push_back(a),
            Job::FinalizationRegistryCleanupJob(j) => {
                self.async_jobs.borrow_mut().push_back(j);
            }
            _ => {}
        }
    }

    fn run_jobs(self: Rc<Self>, context: &mut Context) -> JsResult<()> {
        loop {
            let ran = self.drain(context)?;
            if ran == 0 {
                break;
            }
        }
        Ok(())
    }
}

/// Poll a [`NativeAsyncJob`]'s future to completion with a noop waker.
///
/// The bridge's cooperative, single-threaded futures (module loading via
/// [`crate::core::bridge::TurModuleLoader`], which returns an immediately-
/// ready future) resolve on the first poll, so a noop waker suffices.
fn poll_async_job_to_completion(
    job: NativeAsyncJob,
    context_cell: &RefCell<&mut Context>,
) -> JsResult<()> {
    let future = job.call(context_cell);
    let mut future: Pin<Box<dyn Future<Output = JsResult<boa_engine::JsValue>>>> = Box::pin(future);
    let waker = noop_waker();
    let mut task_cx = TaskContext::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut task_cx) {
            Poll::Ready(res) => {
                res?;
                return Ok(());
            }
            // Single-threaded cooperative futures don't register real wakers;
            // a Pending result means the future yields control. Retry on the
            // next iteration rather than spinning the CPU.
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// A `Waker` that does nothing — module-load futures are self-completing on
/// poll, so no real wake-up machinery is required.
fn noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop_action, noop_action, noop_drop);
    const fn noop_clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    const fn noop_action(_: *const ()) {}
    const fn noop_drop(_: *const ()) {}
    // SAFETY: the vtable functions are noops operating on a null pointer, which is sound.
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}
