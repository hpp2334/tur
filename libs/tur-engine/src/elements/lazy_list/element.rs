use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::property::PropertyDescriptor;
use boa_engine::{js_string, Context, JsString, JsValue};

use crate::core::elements::{ElementJsCallbackEmitter, ElementOnUpdate, ElementOnWheel, ElementOnWheelContext, ElementTrace, WheelEvent};
use crate::core::js_command::{AnyJsCommand, LazyListJsCommand};
use crate::elements::lazy_list::controller::LazyListController;
use crate::elements::scroll_view::ScrollPosition;
use tur_shared::Axis;

#[derive(Clone)]
pub struct LazyListElement {
    pub(crate) axis: Axis,
    pub(crate) item_count: u64,
    pub(crate) item_extent: f64,
    pub(crate) overscan: u64,
    pub(crate) start_index: u64,
    pub(crate) position: ScrollPosition,
    pub(crate) controller: Option<JsObject>,
    pub(crate) reported_start: u64,
    pub(crate) reported_end: u64,
}

impl LazyListElement {
    pub fn new() -> Self {
        LazyListElement {
            axis: Axis::Vertical,
            item_count: 0,
            item_extent: 0.0,
            overscan: 3,
            start_index: 0,
            position: ScrollPosition::new(),
            controller: None,
            reported_start: 0,
            reported_end: 0,
        }
    }

    pub fn scroll_offset(&self) -> f64 {
        self.position.pixels()
    }

    pub fn item_count(&self) -> u64 {
        self.item_count
    }

    pub fn item_extent(&self) -> f64 {
        self.item_extent
    }

    pub fn compute_visible_range(&self, viewport_main: f64) -> (u64, u64) {
        if self.item_count == 0 || self.item_extent <= 0.0 {
            return (0, 0);
        }
        let scroll = self.position.pixels();
        let raw_start = (scroll / self.item_extent).floor() as u64;
        let raw_end = ((scroll + viewport_main) / self.item_extent).ceil() as u64;
        let start = raw_start.saturating_sub(self.overscan);
        let end = (raw_end + self.overscan).min(self.item_count.saturating_sub(1));
        (start, end)
    }

    pub fn update_controller_metrics(&mut self) {
        let Some(ref ctrl_obj) = self.controller else { return };
        let Some(mut ctrl) = ctrl_obj.downcast_mut::<LazyListController>() else {
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
}

impl Default for LazyListElement {
    fn default() -> Self {
        Self::new()
    }
}

impl ElementTrace for LazyListElement {
    fn trace_label(&self) -> String {
        format!(
            "axis={:?} items={} extent={:.1} offset={:.1} range={}-{}",
            self.axis,
            self.item_count,
            self.item_extent,
            self.position.pixels(),
            self.reported_start,
            self.reported_end,
        )
    }
}

impl ElementOnUpdate for LazyListElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        match key.to_std_string_escaped().as_str() {
            "axis" => {
                if let Some(n) = value.as_number() {
                    self.axis = match n as u64 {
                        0 => Axis::Vertical,
                        1 => Axis::Horizontal,
                        _ => Axis::Vertical,
                    };
                }
            }
            "itemCount" => {
                if let Some(n) = value.as_number() {
                    self.item_count = n as u64;
                }
            }
            "itemExtent" => {
                if let Some(n) = value.as_number() {
                    self.item_extent = n;
                }
            }
            "overscan" => {
                if let Some(n) = value.as_number() {
                    self.overscan = n as u64;
                }
            }
            "startIndex" => {
                if let Some(n) = value.as_number() {
                    self.start_index = n as u64;
                }
            }
            "controller" => {
                if let Some(obj) = value.as_object() {
                    self.controller = Some(obj.clone());
                }
            }
            _ => {}
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "axis" => self.axis = Axis::Vertical,
            "itemCount" => self.item_count = 0,
            "itemExtent" => self.item_extent = 0.0,
            "overscan" => self.overscan = 3,
            "startIndex" => self.start_index = 0,
            "controller" => self.controller = None,
            _ => {}
        }
    }
}

impl ElementOnWheel for LazyListElement {
    fn on_wheel(&mut self, cx: &mut ElementOnWheelContext, event: &WheelEvent) -> f64 {
        let delta = match self.axis {
            Axis::Vertical => event.delta_y,
            Axis::Horizontal => event.delta_x,
        };

        let old_pixels = self.position.pixels();
        let overscroll = self.position.apply_scroll_delta(delta);
        let new_pixels = self.position.pixels();

        if (new_pixels - old_pixels).abs() > 0.001 {
            let vp = self.position.viewport_size();
            let viewport_main = self.axis.main(vp);
            let (start, end) = self.compute_visible_range(viewport_main);

            if start != self.reported_start || end != self.reported_end {
                cx.push_js_command(LazyListJsCommand::VisibleRangeDidChange {
                    start_index: start,
                    end_index: end,
                });
                self.reported_start = start;
                self.reported_end = end;
            }

            self.update_controller_metrics();
            cx.push_js_command(LazyListJsCommand::ScrollDidUpdate);
            cx.request_redraw();
        }

        overscroll
    }
}

impl ElementJsCallbackEmitter for LazyListElement {
    fn emit_js_callback(
        &self,
        context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)> {
        let ctrl_obj = self.controller.as_ref()?;
        let ctrl = ctrl_obj.downcast_ref::<LazyListController>()?;

        match command.downcast_ref::<LazyListJsCommand>()? {
            LazyListJsCommand::VisibleRangeDidChange { start_index, end_index } => {
                let on_visible_range_change = ctrl.on_visible_range_change.as_ref()?;

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
                obj.insert_property(js_string!("offset"), desc(self.position.pixels()));
                obj.insert_property(js_string!("maxExtent"), desc(self.position.max_scroll_extent()));
                obj.insert_property(js_string!("viewportDimension"), desc(ctrl.viewport_dimension));
                obj.insert_property(js_string!("startIndex"), desc(*start_index as f64));
                obj.insert_property(js_string!("endIndex"), desc(*end_index as f64));

                Some((on_visible_range_change.clone(), vec![obj.into()]))
            }
            LazyListJsCommand::ScrollDidUpdate => {
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
                obj.insert_property(js_string!("offset"), desc(self.position.pixels()));
                obj.insert_property(js_string!("maxExtent"), desc(self.position.max_scroll_extent()));
                obj.insert_property(js_string!("viewportDimension"), desc(ctrl.viewport_dimension));

                Some((on_scroll.clone(), vec![obj.into()]))
            }
        }
    }
}
