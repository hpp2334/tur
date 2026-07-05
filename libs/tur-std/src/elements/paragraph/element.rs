use boa_engine::object::JsObject;
use boa_engine::Context;

use tur_engine::core::edgy_event::{edgy_mutation_from_js, EdgyMutation};
use tur_engine::core::element::{ElementNodeId, NodeId};
use tur_engine::core::layout::{ElementSubscribe, SubscribeCx};
use tur_engine::core::elements::{
    AnyElement, ComposedGestureEvent, ElementOnFocus, ElementOnGesture,
    ElementOnGestureContext, ElementTrace, TraceValue,
};
use crate::text::SelectionChangeEvent;
use tur_engine::core::view::{
    ViewCx,
    val_from_js, Lifecycle, PropValue, View, Val,
};
use crate::elements::text::span_data::SpanData;
use tur_engine::core::text::text_layout::TextLayoutData;
use tur_shared::Color;

// ---------------------------------------------------------------------------
// TextView — the user's declaration. Pure Rust, no JsValues.
//
// `TextElement` is a leaf element (no children). The `text` and `font_size` props are
// reactive (`Val<T>`); `spans` is parsed eagerly at factory time because each
// span is a composite object (not a primitive the bridge can decode without a
// boa `Context`).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TextView {
    pub text: Option<Val<String>>,
    pub font_size: Option<Val<f64>>,
    /// Default color applied to the anonymous span in the plain-text case.
    pub color: Option<Val<Color>>,
    /// Parsed eagerly at factory time (not reactive).
    pub spans: Option<Vec<SpanData>>,
    pub query_key: Option<Vec<String>>,
    pub on_selection_change: Option<EdgyMutation<SelectionChangeEvent>>,
    /// When `true`, the text can be drag-selected. Defaults to `false`
    /// (read-only, non-selectable) — matches the browser convention for
    /// `<span>`/`<div>` text. Read directly from the spec by the gesture
    /// handler (no reactivity — toggle by rebuilding the element).
    pub selectable: bool,
}

impl View for TextView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::with_gesture_and_focus(TextElement::new(self.clone()))
                .with_callbacks(),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// TextElement — the built element. Holds its spec plus the runtime text-layout cache
// and selection state. Layout/paint read the `Val<T>` props on demand via
// `cx.read_val`.
// ---------------------------------------------------------------------------

pub struct TextElement {
    pub view: TextView,
    pub cached_layout: Option<TextLayoutData>,
    pub cached_spans: Vec<SpanData>,
    pub selection_anchor: usize,
    pub selection_end: usize,
}

impl TextElement {
    pub fn new(spec: TextView) -> Self {
        TextElement {
            view: spec,
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
            self.view.spans.as_deref().unwrap_or(&[])
        }
    }

    fn char_index_at(&self, x: f64, y: f64) -> usize {
        let Some(ref layout) = self.cached_layout else {
            return 0;
        };
        layout.byte_index_at_xy(x as f32, y as f32)
    }
}

impl Lifecycle for TextElement {}

impl ElementSubscribe for TextElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.text.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.font_size.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.color.as_ref() { cx.subscribe_val(v); }
    }
}

impl ElementTrace for TextElement {
    fn trace_label(&self) -> String {
        // Prefer eagerly-parsed spans; fall back to a static `text` prop.
        // Reactive text vals can't be decoded here (no store/Context), so
        // they contribute nothing — same convention as `ContainerElement::trace_label`.
        let text: String = if let Some(spans) = &self.view.spans {
            spans.iter().map(|s| s.text.as_str()).collect()
        } else {
            match &self.view.text {
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
        let c = &self.view;
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
        // Plain Text is non-selectable by default (browser-like). Selection
        // gestures only run when the `selectable` prop was truthy.
        if !self.view.selectable {
            return;
        }
        match event {
            ComposedGestureEvent::PointerDown { local, .. } => {
                cx.request_own_focus();
                let char_idx = self.char_index_at(local.x, local.y);
                self.selection_anchor = char_idx;
                self.selection_end = char_idx;
                let anchor = self.selection_anchor;
                let end = self.selection_end;
                if let Some(m) = self.view.on_selection_change {
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
                    if let Some(m) = self.view.on_selection_change {
                        cx.push_event(m, SelectionChangeEvent { anchor, end });
                    }
                    cx.request_redraw();
                }
            }
            ComposedGestureEvent::PointerUp { .. } => {}
            ComposedGestureEvent::PointerDoubleDown { .. } => {}
            ComposedGestureEvent::PointerTripleDown { .. } => {}
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

impl TextView {
    /// Build a `TextView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        use boa_engine::js_string;
        let on_selection_change = props
            .get(js_string!("onSelectionChange"), ctx)
            .ok()
            .and_then(|v| edgy_mutation_from_js(&v));
        let selectable = props
            .get(js_string!("selectable"), ctx)
            .ok()
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);
        TextView {
            text: prop_val::<String>(props, "text", ctx),
            font_size: prop_val::<f64>(props, "fontSize", ctx),
            color: prop_val::<Color>(props, "color", ctx),
            spans: prop_spans(props, "spans", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
            on_selection_change,
            selectable,
        }
    }
}
