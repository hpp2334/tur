use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::rc::Rc;

use boa_engine::context::time::JsInstant;
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

    pub fn drain(&self, context: &mut Context) -> JsResult<usize> {
        let mut count = 0;

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
            if ran == 0 && self.async_jobs.borrow().is_empty() {
                break;
            }
        }
        Ok(())
    }
}
