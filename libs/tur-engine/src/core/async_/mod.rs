//! Engine-side completion queue + boa job executor.
//!
//! The old `AsyncExecutor` (a future poller + completion queue combined)
//! has been replaced by two focused types:
//! - [`CompletionQueue`] / [`CompletionHandle`] — closures pushed by spawned
//!   futures (which now run on a [`WorkerScheduler`]) to settle JsPromises
//!   under `&mut Context` on the next flush.
//! - [`TurJobExecutor`] — boa's `JobExecutor` impl that drains PromiseJobs /
//!   GenericJobs / AsyncJobs.
//!
//! ## Flush loop
//!
//! ```text
//! loop {
//!     completion_queue.drain(boa);                        // settle promises
//!     jobs_run = executor.drain(boa);                     // run PromiseJobs
//!     // …events, reactive, layout, mutations…
//!     if all quiet { break; }
//! }
//! ```
//!
//! See [`crate::core::app::TurAppInternal::flush`] for the full sequence.

use boa_engine::Context;
use boa_engine::JsResult;

pub mod completion;
pub mod executor;
pub mod flush_tasks;
pub mod task;

pub use completion::{CompletionHandle, CompletionQueue};
pub use executor::TurJobExecutor;
pub use flush_tasks::{FlushTaskHandle, FlushTaskQueue};

/// A closure that runs under `&mut Context` to settle a JsPromise (or any
/// other synchronous side-effect that needs Context access). Produced by a
/// spawned future's completion and drained by [`CompletionQueue::drain`]
/// inside `flush`.
pub type Completion = Box<dyn FnOnce(&mut Context) -> JsResult<()>>;
