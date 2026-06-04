use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::property::PropertyDescriptor;
use boa_engine::{js_string, Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_shared::{Axis, Size};

use crate::core::elements::{
    ElementJsCallbackEmitter, ElementOnUpdate, ElementOnWheel, ElementOnWheelContext,
    ElementTrace, WheelEvent,
};
use crate::core::js_command::{AnyJsCommand, ScrollViewJsCommand};
use crate::core::scroll::ScrollController;
use super::scroll_position::ScrollPosition;

#[derive(Clone)]
pub struct ScrollViewElement {
    pub(crate) axis: Axis,
    pub(crate) position: ScrollPosition,
    pub(crate) controller: Option<JsObject>,
}

impl ScrollViewElement {
    pub fn new() -> Self {
        ScrollViewElement {
            axis: Axis::Vertical,
            position: ScrollPosition::new(),
            controller: None,
        }
    }

    pub fn scroll_offset(&self) -> f64 {
        self.position.pixels()
    }

    pub fn content_size(&self) -> Size {
        self.position.content_size()
    }

    pub fn viewport_size(&self) -> Size {
        self.position.viewport_size()
    }

    pub fn update_controller_metrics(&mut self) {
        let Some(ref ctrl_obj) = self.controller else { return };
        let Some(mut ctrl) = ctrl_obj.downcast_mut::<ScrollController>() else {
            return;
        };
        let vp = self.position.viewport_size();
        let dim = match self.axis {
            Axis::Vertical => vp.height,
            Axis::Horizontal => vp.width,
        };
        ctrl.offset = self.position.pixels();
        ctrl.max_scroll_extent = self.position.max_scroll_extent();
        ctrl.viewport_dimension = dim;
    }

    pub fn apply_pending_initial_offset(&mut self) {
        let Some(ref ctrl_obj) = self.controller else { return };
        let Some(mut ctrl) = ctrl_obj.downcast_mut::<ScrollController>() else {
            return;
        };
        let Some(initial) = ctrl.pending_initial_offset.take() else {
            return;
        };
        let clamped = initial.clamp(0.0, self.position.max_scroll_extent());
        self.position.correct_pixels(clamped);
        ctrl.offset = clamped;
    }
}

impl Default for ScrollViewElement {
    fn default() -> Self {
        Self::new()
    }
}

impl ElementTrace for ScrollViewElement {
    fn trace_label(&self) -> String {
        let vp = self.viewport_size();
        let ct = self.content_size();
        format!(
            "axis={:?} offset={:.1} viewport=({:.1},{:.1}) content=({:.1},{:.1})",
            self.axis,
            self.position.pixels(),
            vp.width,
            vp.height,
            ct.width,
            ct.height,
        )
    }
}

impl ElementOnUpdate for ScrollViewElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "axis" {
            if let Some(n) = value.as_number() {
                self.axis = Axis::from_u64(n as u64).unwrap_or(Axis::Vertical);
            }
        } else if *key == "controller" {
            if let Some(obj) = value.as_object() {
                self.controller = Some(obj.clone());
            }
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "axis" => self.axis = Axis::Vertical,
            "controller" => self.controller = None,
            _ => {}
        }
    }
}

impl ElementOnWheel for ScrollViewElement {
    fn on_wheel(&mut self, cx: &mut ElementOnWheelContext, event: &WheelEvent) -> f64 {
        let delta = match self.axis {
            Axis::Vertical => event.delta_y,
            Axis::Horizontal => event.delta_x,
        };

        let old_pixels = self.position.pixels();
        let overscroll = self.position.apply_scroll_delta(delta);
        let new_pixels = self.position.pixels();

        if (new_pixels - old_pixels).abs() > 0.001 {
            self.update_controller_metrics();
            cx.push_js_command(ScrollViewJsCommand::ScrollDidUpdate);
            cx.request_redraw();
        }

        overscroll
    }
}

impl ElementJsCallbackEmitter for ScrollViewElement {
    fn emit_js_callback(
        &self,
        context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)> {
        let _ = command.downcast_ref::<ScrollViewJsCommand>()?;
        let ctrl_obj = self.controller.as_ref()?;
        let ctrl = ctrl_obj.downcast_ref::<ScrollController>()?;
        let on_scroll = ctrl.on_scroll.as_ref()?;

        let proto = context.intrinsics().constructors().object().prototype();
        let obj = JsObject::from_proto_and_data(proto, ());
        let desc = |v: f64| {
            PropertyDescriptor::builder()
                .value(JsValue::from(v))
                .writable(true)
                .enumerable(true)
                .configurable(true)
                .build()
        };
        obj.insert_property(js_string!("offset"), desc(ctrl.offset));
        obj.insert_property(js_string!("maxExtent"), desc(ctrl.max_scroll_extent));
        obj.insert_property(js_string!("viewportDimension"), desc(ctrl.viewport_dimension));

        Some((on_scroll.clone(), vec![obj.into()]))
    }
}
