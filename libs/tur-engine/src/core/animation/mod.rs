use boa_engine::object::JsObject;

pub mod controller;
pub use controller::AnimationController;

#[derive(Debug, Default)]
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

    pub fn tick_controllers(&mut self, now_ms: u64, ctx: &mut boa_engine::Context) {
        let mut active = Vec::new();
        for obj in self.controllers.drain(..) {
            let keep = {
                let Some(mut ctrl) = obj.downcast_mut::<AnimationController>() else {
                    continue;
                };
                if ctrl.is_active() {
                    ctrl.tick(now_ms, ctx);
                }
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
