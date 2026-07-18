use boa_engine::object::JsObject;

use crate::controller::AnimationController;

/// Registry of active JS `AnimationController`s. Each `AnimationController`
/// registers itself via `forward()` / `reverse()`; the frame loop ticks them
/// and enqueues (does not fire) their `onTick` / `onEnd` callbacks on the
/// mutation queue, which fire later in `flush_pending_mutations` after the
/// `RefMut` on each controller is released.
#[derive(Default)]
pub struct AnimationManager {
    controllers: Vec<JsObject>,
}

impl AnimationManager {
    pub fn new() -> Self {
        AnimationManager {
            controllers: Vec::new(),
        }
    }

    pub fn register_controller(&mut self, obj: JsObject) {
        if !self.controllers.iter().any(|c| c == &obj) {
            self.controllers.push(obj);
        }
    }

    /// Tick all active JS controllers. Each tick updates `value` / `status` and
    /// **enqueues** (does not fire) any `onTick` / `onEnd` callbacks on the
    /// mutation queue. The callbacks fire later in `flush_pending_mutations`,
    /// after the `RefMut` on each controller is released.
    pub fn tick_controllers(&mut self, now_ms: u64, _ctx: &mut boa_engine::Context) {
        let mut active = Vec::new();
        for obj in self.controllers.drain(..) {
            let keep = {
                let Some(mut ctrl) = obj.downcast_mut::<AnimationController>() else {
                    continue;
                };
                let _ = ctrl.tick_compute(now_ms);
                ctrl.is_active()
            };
            if keep {
                active.push(obj);
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
