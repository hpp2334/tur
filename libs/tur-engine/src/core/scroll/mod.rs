pub mod controller;

pub use controller::ScrollController;

use boa_engine::object::JsObject;
use boa_engine::{js_string, Context, JsValue};

use crate::core::widget::callback::EventArg;

// ---------------------------------------------------------------------------
// Scroll event payload — JS callback arguments for onScroll.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ScrollEvent {
    pub offset: f64,
    pub max_extent: f64,
    pub viewport_dimension: f64,
}

impl EventArg for ScrollEvent {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue> {
        let proto = ctx.intrinsics().constructors().object().prototype();
        let obj = JsObject::from_proto_and_data(proto, ());
        let _ = obj.create_data_property_or_throw(
            js_string!("offset"),
            JsValue::from(self.offset),
            ctx,
        );
        let _ = obj.create_data_property_or_throw(
            js_string!("maxExtent"),
            JsValue::from(self.max_extent),
            ctx,
        );
        let _ = obj.create_data_property_or_throw(
            js_string!("viewportDimension"),
            JsValue::from(self.viewport_dimension),
            ctx,
        );
        vec![obj.into()]
    }
}
