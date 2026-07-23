//! The animation subsystem — a [`Subsystem`] that ticks the
//! [`AnimationManager`] once per frame.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::context::time::Clock;
use tur_engine::core::subsystem::{Subsystem, SubsystemFlushContext};

use crate::manager::AnimationManager;

/// Owns the [`AnimationManager`] and ticks it during the engine's flush loop.
///
/// The shared `Rc<RefCell<AnimationManager>>` is also captured by the plugin's
/// `createAnimationController` bridge closure so each newly-constructed
/// `AnimationController` can register itself with the manager on `forward()`
/// / `reverse()`.
///
/// Clock ownership: the subsystem holds its own `Rc<dyn Clock>` (cloned from
/// the engine's clock at register time). It queries `clock.now()` to compute
/// `now_ms` when it advances the manager.
///
/// ## Flush frequency / self-gating
///
/// The engine calls [`Subsystem::flush`] **every fixed-point iteration** of a
/// `TurAppInternal::flush` call (possibly several times per frame). Advancing
/// the controllers, however, must sample the clock **at most once per frame**
/// — otherwise the same frame would re-fire `onTick`/`onEnd` callbacks with
/// recomputed (possibly equal) values. We self-gate that via
/// [`SubsystemFlushContext::frame_id`]: a per-`flush()` epoch that is stable
/// across the iterations of one frame and differs across frames. We advance
/// only when the id changes.
///
/// The schedule signal (`request_next_frame`) is cheap and idempotent, so we
/// emit it on **every** iteration while any controller is active — including
/// iterations where a controller was registered mid-frame (e.g. from an
/// event/lifecycle handler). That is what keeps such an animation advancing
/// without waiting for the next platform input.
pub struct AnimationSubsystem {
    manager: Rc<RefCell<AnimationManager>>,
    clock: Rc<dyn Clock>,
    /// Last `frame_id` we advanced the controllers for. The engine guarantees
    /// `frame_id` is stable across the fixed-point iterations of one
    /// `flush()` call and differs across `flush()` calls, so this gates the
    /// advance to at most once per frame.
    last_frame: u64,
}

impl AnimationSubsystem {
    /// Build a new subsystem. The plugin constructs one shared
    /// `Rc<RefCell<AnimationManager>>`, hands a clone to this constructor,
    /// and hands another clone to the `createAnimationController` closure.
    #[must_use]
    pub fn new(manager: Rc<RefCell<AnimationManager>>, clock: Rc<dyn Clock>) -> Self {
        Self {
            manager,
            clock,
            last_frame: 0,
        }
    }

    /// Borrow the shared manager handle. Used by tests that need to inspect
    /// the active controller count.
    pub fn manager(&self) -> &Rc<RefCell<AnimationManager>> {
        &self.manager
    }
}

impl Subsystem for AnimationSubsystem {
    fn flush(&mut self, cx: &mut SubsystemFlushContext<'_>) {
        // Advance the controllers at most once per frame: only when the
        // engine's frame id has moved on since our last advance. `frame_id`
        // is stable across the fixed-point iterations of one `flush()` call,
        // so repeated iterations within the same frame skip the advance (no
        // double-firing of `onTick`/`onEnd`).
        let id = cx.frame_id();
        if id != self.last_frame {
            self.last_frame = id;
            let now_ms = self.clock.now().millis_since_epoch();
            let mut mgr = self.manager.borrow_mut();
            mgr.tick_controllers(now_ms, cx.boa);
            // Ticking may have enqueued `onTick`/`onEnd` mutations + updated
            // controller values; mark dirty so this frame lays out the new
            // state. (The enqueued mutations also keep the loop iterating.)
            drop(mgr);
            cx.mark_dirty();
        }
        // The schedule signal is emitted on every iteration while any
        // controller is active. This is the key to "animation started from a
        // callback keeps running": a controller registered mid-frame (after
        // the iteration that would have advanced it) is still seen as active
        // here on the very next iteration, so the engine schedules the next
        // vsync and the controller advances on the following frame.
        if self.manager.borrow().has_active() {
            cx.request_next_frame();
        }
    }
}
