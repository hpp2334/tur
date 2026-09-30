use std::rc::Rc;

use boa_engine::object::JsObject;

use crate::controller::AnimationController;

/// A registered controller: a JS class instance (the JS rail) or a
/// Rust-held one (`Rc<RefCell<AnimationController>>`, the rut rail). The
/// frame loop ticks both identically.
#[derive(Clone)]
pub enum ControllerFace {
    Js(JsObject),
    Rust(Rc<std::cell::RefCell<AnimationController>>),
}

/// Registry of active `AnimationController`s. Each `AnimationController`
/// registers itself via `forward()` / `reverse()`; the frame loop ticks them
/// and enqueues (does not fire) their `onTick` / `onEnd` callbacks on the
/// mutation queue, which fire later in `flush_pending_mutations` after the
/// `RefMut` on each controller is released.
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
        let fresh = match &face {
            ControllerFace::Js(obj) => !self
                .controllers
                .iter()
                .any(|c| matches!(c, ControllerFace::Js(o) if o == obj)),
            ControllerFace::Rust(rc) => !self
                .controllers
                .iter()
                .any(|c| matches!(c, ControllerFace::Rust(o) if Rc::ptr_eq(o, rc))),
        };
        if fresh {
            self.controllers.push(face);
        }
    }

    /// Tick all active controllers. Each tick updates `value` / `status` and
    /// **enqueues** (does not fire) any `onTick` / `onEnd` callbacks on the
    /// mutation queue. The callbacks fire later in `flush_pending_mutations`,
    /// after the `RefMut` on each controller is released.
    pub fn tick_controllers(&mut self, now_ms: u64, _ctx: Option<&mut boa_engine::Context>) {
        let mut active = Vec::new();
        for face in self.controllers.drain(..) {
            let keep = match &face {
                ControllerFace::Js(obj) => {
                    let Some(mut ctrl) = obj.downcast_mut::<AnimationController>() else {
                        continue;
                    };
                    let _ = ctrl.tick_compute(now_ms);
                    ctrl.is_active()
                }
                ControllerFace::Rust(rc) => {
                    let Ok(mut ctrl) = rc.try_borrow_mut() else {
                        // A re-entrant borrow (a row driving the controller
                        // mid-tick) skips this frame — the same
                        // single-thread discipline as the Js arm's RefMut.
                        continue;
                    };
                    let _ = ctrl.tick_compute(now_ms);
                    ctrl.is_active()
                }
            };
            if keep {
                active.push(face);
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
