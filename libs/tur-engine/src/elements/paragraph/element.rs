use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::{
    ComposedGestureEvent, ElementJsCallbackEmitter, ElementOnFocus, ElementOnGesture,
    ElementOnGestureContext, ElementOnUpdate, ElementTrace,
};
use crate::core::js_command::{AnyJsCommand, IntoAnyJsCommand};
use crate::elements::text::span_data::SpanData;
use crate::elements::text::text_layout::TextLayoutData;

fn extract_callable(value: &JsValue) -> Option<JsFunction> {
    value.as_object().and_then(JsFunction::from_object)
}

pub struct ParagraphElement {
    pub(crate) spans: Vec<SpanData>,
    pub(crate) default_font_size: f64,
    pub(crate) cached_layout: Option<TextLayoutData>,
    pub(crate) selection_anchor: usize,
    pub(crate) selection_end: usize,
    on_selection_change: Option<JsFunction>,
}

impl Default for ParagraphElement {
    fn default() -> Self {
        Self::new()
    }
}

impl ParagraphElement {
    pub fn new() -> Self {
        ParagraphElement {
            spans: Vec::new(),
            default_font_size: 14.0,
            cached_layout: None,
            selection_anchor: 0,
            selection_end: 0,
            on_selection_change: None,
        }
    }

    pub fn spans(&self) -> &[SpanData] {
        &self.spans
    }

    fn char_index_at(&self, x: f64, y: f64) -> usize {
        let Some(ref layout) = self.cached_layout else {
            return 0;
        };
        layout.char_index_at_xy(x as f32, y as f32)
    }
}

impl ElementTrace for ParagraphElement {
    fn trace_label(&self) -> String {
        let text: String = self.spans.iter().map(|s| s.text.as_str()).collect();
        if text.is_empty() {
            String::new()
        } else {
            format!("\"{}\"", if text.len() > 20 { &text[..20] } else { &text })
        }
    }
}

impl ElementOnUpdate for ParagraphElement {
    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue) {
        match key.to_std_string_escaped().as_str() {
            "spans" => {
                self.spans = crate::elements::text::span_data::extract_spans_from_js(value, ctx);
            }
            "fontSize" => {
                self.default_font_size = value.as_number().unwrap_or(14.0);
            }
            "onSelectionChange" => {
                self.on_selection_change = extract_callable(value);
            }
            _ => {}
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "spans" => self.spans.clear(),
            "fontSize" => self.default_font_size = 14.0,
            "onSelectionChange" => self.on_selection_change = None,
            _ => {}
        }
    }
}

impl ElementOnFocus for ParagraphElement {}

impl ElementOnGesture for ParagraphElement {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) -> bool {
        match event {
            ComposedGestureEvent::PointerDown { local_position } => {
                cx.request_own_focus();
                let char_idx = self.char_index_at(local_position.x, local_position.y);
                self.selection_anchor = char_idx;
                self.selection_end = char_idx;
                cx.push_js_command(ParagraphJsCommand::SelectionChanged {
                    anchor: self.selection_anchor,
                    end: self.selection_end,
                });
                cx.request_redraw();
            }
            ComposedGestureEvent::PointerMove { local_position } => {
                let char_idx = self.char_index_at(local_position.x, local_position.y);
                if char_idx != self.selection_end {
                    self.selection_end = char_idx;
                    cx.push_js_command(ParagraphJsCommand::SelectionChanged {
                        anchor: self.selection_anchor,
                        end: self.selection_end,
                    });
                    cx.request_redraw();
                }
            }
            ComposedGestureEvent::Wheel { .. } => {}
        }
        false
    }
}

#[derive(Clone)]
pub(crate) enum ParagraphJsCommand {
    SelectionChanged { anchor: usize, end: usize },
}

impl IntoAnyJsCommand for ParagraphJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand {
        AnyJsCommand(std::rc::Rc::new(self))
    }
}

impl ElementJsCallbackEmitter for ParagraphElement {
    fn emit_js_callback(
        &self,
        _context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)> {
        if let Some(c) = command.downcast_ref::<ParagraphJsCommand>() {
            match c {
                ParagraphJsCommand::SelectionChanged { anchor, end } => {
                    self.on_selection_change.as_ref().map(|h| {
                        (
                            h.clone(),
                            vec![
                                boa_engine::JsValue::from(*anchor as f64),
                                boa_engine::JsValue::from(*end as f64),
                            ],
                        )
                    })
                }
            }
        } else {
            None
        }
    }
}
