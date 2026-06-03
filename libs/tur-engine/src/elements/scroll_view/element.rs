use boa_engine::{Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_shared::{Axis, Size};

use crate::core::elements::{ElementOnUpdate, ElementOnWheel, ElementOnWheelContext, ElementTrace, WheelEvent};
use super::scroll_position::ScrollPosition;

#[derive(Clone)]
pub struct ScrollViewElement {
    pub(crate) axis: Axis,
    pub(crate) position: ScrollPosition,
}

impl ScrollViewElement {
    pub fn new() -> Self {
        ScrollViewElement {
            axis: Axis::Vertical,
            position: ScrollPosition::new(),
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
        } else if *key == "scrollOffset" {
            if let Some(n) = value.as_number() {
                self.position.correct_pixels(n);
            }
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "axis" => self.axis = Axis::Vertical,
            "scrollOffset" => self.position.correct_pixels(0.0),
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
            cx.request_redraw();
        }

        overscroll
    }
}
