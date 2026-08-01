pub mod controller;

pub use controller::ScrollController;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue, js_string};

use crate::core::edgy::mutation::IntoJsArgs;

// ---------------------------------------------------------------------------
// Scroll event payload — JS callback arguments for onScroll.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ScrollEvent {
    pub(crate) offset: f64,
    pub(crate) max_extent: f64,
    pub(crate) viewport_dimension: f64,
}

impl ScrollEvent {
    /// Construct a scroll event payload from the controller's live metrics.
    pub fn new(offset: f64, max_extent: f64, viewport_dimension: f64) -> Self {
        Self {
            offset,
            max_extent,
            viewport_dimension,
        }
    }
}

impl IntoJsArgs for ScrollEvent {
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
