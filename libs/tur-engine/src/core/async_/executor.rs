use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context as TaskContext, Poll, RawWaker, RawWakerVTable, Waker};

use boa_engine::Context;
use boa_engine::JsResult;
use boa_engine::job::{GenericJob, Job, JobExecutor, NativeAsyncJob, PromiseJob};

#[derive(Default)]
pub struct TurJobExecutor {
    promise_jobs: RefCell<VecDeque<PromiseJob>>,
    generic_jobs: RefCell<VecDeque<GenericJob>>,
    async_jobs: RefCell<VecDeque<NativeAsyncJob>>,
}

impl TurJobExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn drain(&self, context: &mut Context) -> JsResult<usize> {
        let mut count = 0;

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
    fn enqueue_job(self: Rc<Self>, job: Job, _context: &mut Context) {
        match job {
            Job::PromiseJob(p) => self.promise_jobs.borrow_mut().push_back(p),
            Job::GenericJob(g) => self.generic_jobs.borrow_mut().push_back(g),
            Job::AsyncJob(a) => self.async_jobs.borrow_mut().push_back(a),
            Job::FinalizationRegistryCleanupJob(j) => {
                self.async_jobs.borrow_mut().push_back(j);
            }
            // `TimeoutJob` / `IntervalJob` are produced only by host-provided
            // `setTimeout`/`setInterval` — tur no longer registers those
            // (replaced by the `sleep` + `launch` task primitives), so these
            // variants are never enqueued. Drop them.
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
/// [`crate::core::js_runtime::TurModuleLoader`], which returns an immediately-
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
    const VTABLE: RawWakerVTable =
        RawWakerVTable::new(noop_clone, noop_action, noop_action, noop_drop);
    const fn noop_clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    const fn noop_action(_: *const ()) {}
    const fn noop_drop(_: *const ()) {}
    // SAFETY: the vtable functions are noops operating on a null pointer, which is sound.
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}
