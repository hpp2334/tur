//! The animation subsystem — a [`Subsystem`] that ticks the
//! [`AnimationManager`] once per frame.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::context::time::Clock;
use tur_engine::core::subsystem::{Subsystem, SubsystemFlushContext, SubsystemOutcome};

use crate::manager::AnimationManager;

/// Owns the [`AnimationManager`] and ticks it once per `TurAppInternal::flush`
/// call (= once per frame).
///
/// The shared `Rc<RefCell<AnimationManager>>` is also captured by the plugin's
/// `createAnimationController` bridge closure so each newly-constructed
/// `AnimationController` can register itself with the manager on `forward()`
/// / `reverse()`.
///
/// Clock ownership: the subsystem holds its own `Rc<dyn Clock>` (cloned from
/// the engine's clock at register time). It queries `clock.now()` during
/// `flush` to compute `now_ms` for the manager tick.
pub struct AnimationSubsystem {
    manager: Rc<RefCell<AnimationManager>>,
    clock: Rc<dyn Clock>,
}

impl AnimationSubsystem {
    /// Build a new subsystem. The plugin constructs one shared
    /// `Rc<RefCell<AnimationManager>>`, hands a clone to this constructor,
    /// and hands another clone to the `createAnimationController` closure.
    #[must_use]
    pub fn new(manager: Rc<RefCell<AnimationManager>>, clock: Rc<dyn Clock>) -> Self {
        Self { manager, clock }
    }

    /// Borrow the shared manager handle. Used by tests that need to inspect
    /// the active controller count.
    pub fn manager(&self) -> &Rc<RefCell<AnimationManager>> {
        &self.manager
    }
}

impl Subsystem for AnimationSubsystem {
    fn flush(&mut self, cx: &mut SubsystemFlushContext<'_>) -> SubsystemOutcome {
        let now_ms = self.clock.now().millis_since_epoch();
        let active = {
            let mut mgr = self.manager.borrow_mut();
            mgr.tick_controllers(now_ms, cx.boa);
            mgr.has_active()
        };
        // Both `dirtied` and `request_frame` track `has_active`:
        //   - `dirtied` so the loop continues if any controller ticked (and
        //     enqueued `onTick` mutations that need to fire in a subsequent
        //     `flush_pending_mutations`);
        //   - `request_frame` so the engine schedules the next vsync while
        //     any controller is still running.
        SubsystemOutcome::from_active(active)
    }
}
