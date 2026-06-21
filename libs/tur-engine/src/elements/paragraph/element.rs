use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::edgy_event::{edgy_mutation_from_js, EdgyMutation};
use crate::core::element::ElementNodeId;
use crate::core::elements::{
    AnyElement, ComposedGestureEvent, ElementOnFocus, ElementOnGesture,
    ElementOnGestureContext, ElementTrace, TraceValue,
};
use crate::core::text::SelectionChangeEvent;
use crate::core::widget::{
    val_from_js, Effect, PropValue, Component, Val, WidgetCx,
};
use crate::elements::text::span_data::SpanData;
use crate::elements::text::text_layout::TextLayoutData;
use tur_shared::Color;

// ---------------------------------------------------------------------------
// TextComponent — the user's declaration. Pure Rust, no JsValues.
//
// `TextElement` is a leaf element (no children). The `text` and `font_size` props are
// reactive (`Val<T>`); `spans` is parsed eagerly at factory time because each
// span is a composite object (not a primitive the bridge can decode without a
// boa `Context`).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TextComponent {
    pub text: Option<Val<String>>,
    pub font_size: Option<Val<f64>>,
    /// Default color applied to the anonymous span in the plain-text case.
    pub color: Option<Val<Color>>,
    /// Parsed eagerly at factory time (not reactive).
    pub spans: Option<Vec<SpanData>>,
    pub query_key: Option<Vec<String>>,
    pub on_selection_change: Option<EdgyMutation<SelectionChangeEvent>>,
}

impl Component for TextComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::with_gesture_and_focus(TextElement::new(self.clone()))
                .with_callbacks(),
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
// TextElement — the built element. Holds its spec plus the runtime text-layout cache
// and selection state. Layout/paint read the `Val<T>` props on demand via
// `cx.read_val`.
// ---------------------------------------------------------------------------

pub struct TextElement {
    pub component: TextComponent,
    pub(crate) cached_layout: Option<TextLayoutData>,
    pub(crate) cached_spans: Vec<SpanData>,
    pub(crate) selection_anchor: usize,
    pub(crate) selection_end: usize,
}

impl TextElement {
    pub fn new(spec: TextComponent) -> Self {
        TextElement {
            component: spec,
            cached_layout: None,
            cached_spans: Vec::new(),
            selection_anchor: 0,
            selection_end: 0,
        }
    }

    pub fn spans(&self) -> &[SpanData] {
        if !self.cached_spans.is_empty() {
            &self.cached_spans
        } else {
            self.component.spans.as_deref().unwrap_or(&[])
        }
    }

    fn char_index_at(&self, x: f64, y: f64) -> usize {
        let Some(ref layout) = self.cached_layout else {
            return 0;
        };
        layout.byte_index_at_xy(x as f32, y as f32)
    }
}

impl Effect for TextElement {}

impl ElementTrace for TextElement {
    fn trace_label(&self) -> String {
        // Prefer eagerly-parsed spans; fall back to a static `text` prop.
        // Reactive text vals can't be decoded here (no store/Context), so
        // they contribute nothing — same convention as `ContainerElement::trace_label`.
        let text: String = if let Some(spans) = &self.component.spans {
            spans.iter().map(|s| s.text.as_str()).collect()
        } else {
            match &self.component.text {
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

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let c = &self.component;
        let mut p = Vec::new();
        if let Some(spans) = &c.spans {
            let text: String = spans.iter().map(|s| s.text.as_str()).collect();
            p.push(("text", TraceValue::Str(text)));
        } else if let Some(v) = c.text.as_ref().and_then(Val::as_static) {
            p.push(("text", TraceValue::Str(v.clone())));
        }
        if let Some(v) = c.font_size.as_ref().and_then(Val::as_static) {
            p.push(("fontSize", TraceValue::Num(*v)));
        }
        p
    }
}

impl ElementOnFocus for TextElement {}

impl ElementOnGesture for TextElement {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) {
        match event {
            ComposedGestureEvent::PointerDown { local, .. } => {
                cx.request_own_focus();
                let char_idx = self.char_index_at(local.x, local.y);
                self.selection_anchor = char_idx;
                self.selection_end = char_idx;
                let anchor = self.selection_anchor;
                let end = self.selection_end;
                if let Some(m) = self.component.on_selection_change {
                    cx.push_event(m, SelectionChangeEvent { anchor, end });
                }
                cx.request_redraw();
            }
            ComposedGestureEvent::PointerMove { local, .. } => {
                let char_idx = self.char_index_at(local.x, local.y);
                if char_idx != self.selection_end {
                    self.selection_end = char_idx;
                    let anchor = self.selection_anchor;
                    let end = self.selection_end;
                    if let Some(m) = self.component.on_selection_change {
                        cx.push_event(m, SelectionChangeEvent { anchor, end });
                    }
                    cx.request_redraw();
                }
            }
            ComposedGestureEvent::PointerUp { .. } => {}
            ComposedGestureEvent::ContextMenu { .. } => {}
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

impl TextComponent {
    /// Build a `TextComponent` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        use boa_engine::js_string;
        let on_selection_change = props
            .get(js_string!("onSelectionChange"), ctx)
            .ok()
            .and_then(|v| edgy_mutation_from_js(&v));
        TextComponent {
            text: prop_val::<String>(props, "text", ctx),
            font_size: prop_val::<f64>(props, "fontSize", ctx),
            color: prop_val::<Color>(props, "color", ctx),
            spans: prop_spans(props, "spans", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
            on_selection_change,
        }
    }
}
