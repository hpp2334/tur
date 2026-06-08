use boa_engine::{Context, JsString, JsValue};
use tur_shared::AnimatableValue;

pub trait ElementOnUpdate: 'static {
    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue);
    fn reset_prop(&mut self, key: &JsString);
    fn apply_animated(&mut self, _key: &str, _value: AnimatableValue) {}
    fn get_animatable(&self, _key: &str) -> Option<AnimatableValue> {
        None
    }
}
