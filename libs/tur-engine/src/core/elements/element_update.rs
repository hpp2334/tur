use boa_engine::{Context, JsString, JsValue};

pub trait ElementOnUpdate: Send + Sync + 'static {
    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue);
}
