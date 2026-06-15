use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::element::ElementNodeId;
use crate::core::elements::{
    AnyElement, ComposedGestureEvent, ElementJsCallbackEmitter, ElementOnFocus, ElementOnGesture,
    ElementOnGestureContext, ElementTrace,
};
use crate::core::js_command::{AnyJsCommand, IntoAnyJsCommand};
use crate::core::widget::{val_from_js, Effect, PropValue, Spec, Val, WidgetCx};
use crate::elements::text::span_data::SpanData;
use crate::elements::text::text_layout::TextLayoutData;
use tur_shared::Color;

// ---------------------------------------------------------------------------
// TextSpec — the user's declaration. Pure Rust, no JsValues.
//
// `Text` is a leaf element (no children). The `text` and `font_size` props are
// reactive (`Val<T>`); `spans` is parsed eagerly at factory time because each
// span is a composite object (not a primitive the bridge can decode without a
// boa `Context`).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TextSpec {
    pub text: Option<Val<String>>,
    pub font_size: Option<Val<f64>>,
    /// Default color applied to the anonymous span in the plain-text case.
    pub color: Option<Val<Color>>,
    /// Parsed eagerly at factory time (not reactive).
    pub spans: Option<Vec<SpanData>>,
    pub query_key: Option<Vec<String>>,
}

impl Spec for TextSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::with_gesture_and_focus(Text::new(self.clone()))
                .with_js_callback_emitter::<Text>(),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// Text — the built element. Holds its spec plus the runtime text-layout cache
// and selection state. Layout/paint read the `Val<T>` props on demand via
// `cx.read_val`.
// ---------------------------------------------------------------------------

pub struct Text {
    pub spec: TextSpec,
    pub(crate) cached_layout: Option<TextLayoutData>,
    pub(crate) cached_spans: Vec<SpanData>,
    pub(crate) selection_anchor: usize,
    pub(crate) selection_end: usize,
    on_selection_change: Option<JsFunction>,
}

impl Text {
    pub fn new(spec: TextSpec) -> Self {
        Text {
            spec,
            cached_layout: None,
            cached_spans: Vec::new(),
            selection_anchor: 0,
            selection_end: 0,
            on_selection_change: None,
        }
    }

    pub fn spans(&self) -> &[SpanData] {
        if !self.cached_spans.is_empty() {
            &self.cached_spans
        } else {
            self.spec.spans.as_deref().unwrap_or(&[])
        }
    }

    pub fn set_on_selection_change(&mut self, handler: Option<JsFunction>) {
        self.on_selection_change = handler;
    }

    fn char_index_at(&self, x: f64, y: f64) -> usize {
        let Some(ref layout) = self.cached_layout else {
            return 0;
        };
        layout.char_index_at_xy(x as f32, y as f32)
    }
}

impl Effect for Text {}

impl ElementTrace for Text {
    fn trace_label(&self) -> String {
        // Prefer eagerly-parsed spans; fall back to a static `text` prop.
        // Reactive text vals can't be decoded here (no store/Context), so
        // they contribute nothing — same convention as `Container::trace_label`.
        let text: String = if let Some(spans) = &self.spec.spans {
            spans.iter().map(|s| s.text.as_str()).collect()
        } else {
            match &self.spec.text {
                Some(Val::Static(s)) => s.clone(),
                _ => String::new(),
            }
        };
        if text.is_empty() {
            String::new()
        } else {
            let head: String = text.chars().take(20).collect();
            format!("\"{head}\"")
        }
    }
}

impl ElementOnFocus for Text {}

impl ElementOnGesture for Text {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) {
        match event {
            ComposedGestureEvent::PointerDown { local_position } => {
                cx.request_own_focus();
                let char_idx = self.char_index_at(local_position.x, local_position.y);
                self.selection_anchor = char_idx;
                self.selection_end = char_idx;
                cx.push_js_command(TextJsCommand::SelectionChanged {
                    anchor: self.selection_anchor,
                    end: self.selection_end,
                });
                cx.request_redraw();
            }
            ComposedGestureEvent::PointerMove { local_position } => {
                let char_idx = self.char_index_at(local_position.x, local_position.y);
                if char_idx != self.selection_end {
                    self.selection_end = char_idx;
                    cx.push_js_command(TextJsCommand::SelectionChanged {
                        anchor: self.selection_anchor,
                        end: self.selection_end,
                    });
                    cx.request_redraw();
                }
            }
        }
    }
}

#[derive(Clone)]
pub(crate) enum TextJsCommand {
    SelectionChanged { anchor: usize, end: usize },
}

impl IntoAnyJsCommand for TextJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand {
        AnyJsCommand(std::rc::Rc::new(self))
    }
}

impl ElementJsCallbackEmitter for Text {
    fn emit_js_callback(
        &self,
        _context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)> {
        let c = command.downcast_ref::<TextJsCommand>()?;
        match c {
            TextJsCommand::SelectionChanged { anchor, end } => {
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
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

/// Extract a `Val<T>` prop from a JS props object.
fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

/// Extract a `Vec<String>` prop (queryKey) — parsed eagerly.
fn prop_query_key(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Vec<String>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    let obj = v.as_object()?;
    let arr = JsArray::from_object(obj.clone()).ok()?;
    let len = arr.length(ctx).ok()? as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if let Ok(val) = arr.at(i as i64, ctx) {
            if let Some(s) = val.as_string() {
                out.push(s.to_std_string_escaped());
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Extract the `spans` array — parsed eagerly into `Vec<SpanData>`.
fn prop_spans(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Vec<SpanData>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    if v.is_null() || v.is_undefined() {
        return None;
    }
    let parsed = crate::elements::text::span_data::extract_spans_from_js(&v, ctx);
    if parsed.is_empty() { None } else { Some(parsed) }
}

impl TextSpec {
    /// Build a `TextSpec` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        TextSpec {
            text: prop_val::<String>(props, "text", ctx),
            font_size: prop_val::<f64>(props, "fontSize", ctx),
            color: prop_val::<Color>(props, "color", ctx),
            spans: prop_spans(props, "spans", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
