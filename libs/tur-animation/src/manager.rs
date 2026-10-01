use std::rc::Rc;

use crate::controller::AnimationController;

/// A registered controller handle (`Rc<RefCell<AnimationController>>`).
/// The frame loop ticks every registered controller identically.
pub type ControllerFace = Rc<std::cell::RefCell<AnimationController>>;

/// Registry of active `AnimationController`s. Each controller registers
/// itself via `forward()` / `reverse()` (the `tur` pkg rows); the frame loop
/// ticks them and enqueues (does not fire) their `onTick` / `onEnd`
/// callbacks on the mutation queue, which fire later in
/// `flush_pending_mutations` after the `RefMut` on each controller is
/// released.
#[derive(Default)]
pub struct AnimationManager {
    controllers: Vec<ControllerFace>,
}

impl AnimationManager {
    pub fn new() -> Self {
        AnimationManager {
            controllers: Vec::new(),
        }
    }

    pub fn register_controller(&mut self, face: ControllerFace) {
        let fresh = !self
            .controllers
            .iter()
            .any(|c| Rc::ptr_eq(c, &face));
        if fresh {
            self.controllers.push(face);
        }
    }

    /// Tick all active controllers. Each tick updates `value` / `status` and
    /// **enqueues** (does not fire) any `onTick` / `onEnd` callbacks on the
    /// mutation queue. The callbacks fire later in `flush_pending_mutations`,
    /// after the `RefMut` on each controller is released.
    pub fn tick_controllers(&mut self, now_ms: u64) {
        let mut active = Vec::new();
        for rc in self.controllers.drain(..) {
            // A re-entrant borrow (a row driving the controller mid-tick)
            // skips this frame — the single-thread borrow discipline.
            let Ok(mut ctrl) = rc.try_borrow_mut() else {
                continue;
            };
            let keep = {
                let _ = ctrl.tick_compute(now_ms);
                ctrl.is_active()
            };
            drop(ctrl);
            if keep {
                active.push(rc);
            }
        }
        self.controllers = active;
    }

    pub fn has_active(&self) -> bool {
        !self.controllers.is_empty()
    }
}

impl std::fmt::Debug for AnimationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimationManager")
            .field("controllers", &self.controllers.len())
            .finish()
    }
}
