use boa_engine::{Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_shared::{Axis, Size};

use crate::core::elements::{ComposedGestureEvent, ElementOnGesture, ElementOnGestureContext, ElementOnUpdate, ElementTrace};

#[derive(Clone)]
pub struct ScrollViewElement {
    pub(crate) axis: Axis,
    pub(crate) scroll_offset: f64,
    pub(crate) viewport_size: Size,
    pub(crate) content_size: Size,
}

impl ScrollViewElement {
    pub fn new() -> Self {
        ScrollViewElement {
            axis: Axis::Vertical,
            scroll_offset: 0.0,
            viewport_size: Size::ZERO,
            content_size: Size::ZERO,
        }
    }

    pub fn scroll_offset(&self) -> f64 {
        self.scroll_offset
    }

    pub fn content_size(&self) -> Size {
        self.content_size
    }

    pub fn viewport_size(&self) -> Size {
        self.viewport_size
    }
}

impl Default for ScrollViewElement {
    fn default() -> Self {
        Self::new()
    }
}

impl ElementTrace for ScrollViewElement {
    fn trace_label(&self) -> String {
        format!(
            "axis={:?} offset={:.1} viewport=({:.1},{:.1}) content=({:.1},{:.1})",
            self.axis,
            self.scroll_offset,
            self.viewport_size.width,
            self.viewport_size.height,
            self.content_size.width,
            self.content_size.height,
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
                self.scroll_offset = n;
            }
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "axis" => self.axis = Axis::Vertical,
            "scrollOffset" => self.scroll_offset = 0.0,
            _ => {}
        }
    }
}

impl ElementOnGesture for ScrollViewElement {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) -> bool {
        let ComposedGestureEvent::Wheel { delta_x, delta_y } = event else {
            return false;
        };

        let delta = match self.axis {
            Axis::Vertical => delta_y,
            Axis::Horizontal => delta_x,
        };

        let max_scroll = (self.axis.main(self.content_size) - self.axis.main(self.viewport_size)).max(0.0);
        let new_offset = (self.scroll_offset + delta).clamp(0.0, max_scroll);

        if (new_offset - self.scroll_offset).abs() < 0.001 {
            return false;
        }

        self.scroll_offset = new_offset;
        cx.request_redraw();
        true
    }
}
