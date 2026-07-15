use std::future::Future;
use std::pin::Pin;
use std::rc::Weak;
use std::task::{Context as TaskContext, Poll};
use std::time::Duration;

use crate::{Clock, TimerQueue};

/// A future that completes after `duration` has elapsed.
///
/// Created by [`Executor::sleep`]. On first poll, registers its waker in the
/// executor's timer queue keyed by the absolute deadline. When `tick()` runs
/// and the deadline has passed, the waker is fired and the task is
/// re-enqueued for polling.
///
/// Stale entries (from dropped or cancelled tasks) remain in the timer queue
/// until their deadline passes; they are harmlessly woken and skipped by
/// `tick()` (the task id is no longer in the task map).
pub struct Sleep {
    pub(crate) deadline: Duration,
    pub(crate) timers: TimerQueue,
    pub(crate) clock: Weak<dyn Clock>,
    pub(crate) registered: bool,
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<()> {
        if let Some(clock) = self.clock.upgrade() {
            if clock.now() >= self.deadline {
                return Poll::Ready(());
            }
        } else {
            return Poll::Ready(());
        }
        if !self.registered {
            self.registered = true;
            self.timers
                .borrow_mut()
                .entry(self.deadline)
                .or_default()
                .push(cx.waker().clone());
        }
        Poll::Pending
    }
}
