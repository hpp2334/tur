use boa_engine::{Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_shared::BoxFit;

use crate::core::elements::ElementOnUpdate;
use crate::core::elements::ElementTrace;
use crate::core::resource::ResourceId;

#[derive(Clone, Default)]
pub struct SvgElement {
    pub(crate) resource_id: Option<ResourceId>,
    pub(crate) width: Option<f64>,
    pub(crate) height: Option<f64>,
    pub(crate) fit: BoxFit,
}

impl SvgElement {
    pub fn new() -> Self {
        SvgElement {
            resource_id: None,
            width: None,
            height: None,
            fit: BoxFit::default(),
        }
    }
}

impl ElementTrace for SvgElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(rid) = self.resource_id {
            parts.push(format!("resource={}", rid.as_u64()));
        }
        if let Some(w) = self.width {
            parts.push(format!("width={w}"));
        }
        if let Some(h) = self.height {
            parts.push(format!("height={h}"));
        }
        parts.push(format!("fit={:?}", self.fit));
        parts.join(" ")
    }
}

impl ElementOnUpdate for SvgElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "resourceId" {
            self.resource_id = value.as_number().map(|id| ResourceId::new(id as u64));
        } else if *key == "width" {
            self.width = value.as_number();
        } else if *key == "height" {
            self.height = value.as_number();
        } else if *key == "fit" {
            if let Some(n) = value.as_number() {
                self.fit = BoxFit::from_u64(n as u64).unwrap_or_default();
            }
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "resourceId" => self.resource_id = None,
            "width" => self.width = None,
            "height" => self.height = None,
            "fit" => self.fit = BoxFit::default(),
            _ => {}
        }
    }
}
