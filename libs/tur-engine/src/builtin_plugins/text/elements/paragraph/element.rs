use boa_engine::object::JsObject;
use boa_engine::{Context, JsError, JsValue};

use crate::builtin_plugins::text::controller::SelectionChangeEvent;
use crate::builtin_plugins::text::elements::text_shared::span_data::SpanData;
use crate::core::edgy::mutation::MutationHandle;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{
    AnyElement, ComposedGestureEvent, ElementOnFocus, ElementOnGesture, ElementOnGestureContext,
    ElementTrace, TraceValue,
};
use crate::core::js_runtime::JsProps;
use crate::core::js_runtime::js_value::{FromJs, type_error};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::platform::PointerDeviceKind;
use crate::core::render::brush::Color;
use crate::core::text::text_layout::TextLayoutData;
use crate::core::view::{Lifecycle, Val, View, ViewCx};

/// How `Text` handles content that exceeds [`TextView::max_lines`].
///
/// Mirrors Flutter's `TextOverflow`:
/// - `Clip`     — render at most `maxLines` lines, discard the rest.
/// - `Ellipsis` — render at most `maxLines` lines, append an ellipsis (`…`)
///   to the last visible line (trimmed to fit).
/// - `Visible`  — render all lines; `maxLines` is ignored.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum TextOverflow {
    #[default]
    Clip,
    Ellipsis,
    Visible,
}

impl FromJs for TextOverflow {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        let s = v
            .as_string()
            .ok_or_else(|| type_error("a TextOverflow keyword string"))?;
        match s.to_std_string_escaped().as_str() {
            "clip" => Ok(TextOverflow::Clip),
            "ellipsis" => Ok(TextOverflow::Ellipsis),
            "visible" => Ok(TextOverflow::Visible),
            _ => Err(type_error(
                "a recognized TextOverflow keyword (clip|ellipsis|visible)",
            )),
        }
    }
}

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
    pub(crate) text: Option<Val<String>>,
    pub(crate) font_size: Option<Val<f64>>,
    /// Default color applied to the anonymous span in the plain-text case.
    pub(crate) color: Option<Val<Color>>,
    /// Parsed eagerly at factory time (not reactive).
    pub(crate) spans: Option<Vec<SpanData>>,
    pub(crate) query_key: Option<Vec<String>>,
    pub(crate) on_selection_change: Option<MutationHandle<SelectionChangeEvent>>,
    /// When `true`, the text can be drag-selected. Defaults to `false`
    /// (read-only, non-selectable) — matches the browser convention for
    /// `<span>`/`<div>` text. Read directly from the spec by the gesture
    /// handler (no reactivity — toggle by rebuilding the element).
    pub(crate) selectable: bool,
    /// Maximum number of lines to render. `None` (or `0`) means no limit.
    /// Honored when [`TextView::overflow`] is `Clip` or `Ellipsis`; ignored
    /// when it is `Visible`.
    pub(crate) max_lines: Option<Val<u32>>,
    /// How content past [`TextView::max_lines`] is handled. Defaults to
    /// `Clip` (matches Flutter's `TextOverflow.clip`).
    pub(crate) overflow: Option<Val<TextOverflow>>,
}

impl View for TextView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::with_gesture_and_focus(TextElement::new(self.clone())).with_callbacks(),
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
    pub(crate) view: TextView,
    pub(crate) cached_layout: Option<TextLayoutData>,
    pub(crate) cached_spans: Vec<SpanData>,
    pub(crate) selection_anchor: usize,
    pub(crate) selection_end: usize,
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
        if let Some(v) = c.text.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.font_size.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.color.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.max_lines.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.overflow.as_ref() {
            cx.subscribe_val(v);
        }
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
    fn accepts_device(&self, _device: PointerDeviceKind) -> bool {
        // Non-selectable text rejects all devices so the touch-slop probe
        // falls through to gesture-capable elements beneath (e.g. a wrapping
        // `PointerInteract`). Without this, a static `Text` painted on top of
        // a `PointerInteract` would steal touch drags: the piece's number
        // `Text` won the probe and the `PointerInteract` beneath never saw
        // `onPointerDown` (the drag appeared dead on touch, while mouse drags
        // — which bypass the probe — worked).
        self.view.selectable
    }

    fn on_gesture_event(&mut self, cx: &mut ElementOnGestureContext, event: &ComposedGestureEvent) {
        // Mouse/Click dispatch bypasses accepts_device; bail for non-selectable.
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
                cx.request_paint();
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
                    cx.request_paint();
                }
            }
            ComposedGestureEvent::PointerUp { .. } => {}
            ComposedGestureEvent::PointerDoubleDown { .. } => {}
            ComposedGestureEvent::PointerTripleDown { .. } => {}
            ComposedGestureEvent::Click { .. } => {}
            ComposedGestureEvent::ContextMenu { .. } => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

/// Extract the `spans` array — parsed eagerly into `Vec<SpanData>`.
fn prop_spans(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Vec<SpanData>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    if v.is_null() || v.is_undefined() {
        return None;
    }
    let parsed =
        crate::builtin_plugins::text::elements::text_shared::span_data::extract_spans_from_js(
            &v, ctx,
        );
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

impl TextView {
    /// Build a `TextView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let (on_selection_change, selectable) = {
            let mut p = JsProps::new(props, ctx);
            (
                p.mutation::<SelectionChangeEvent>("onSelectionChange"),
                p.opt::<bool>("selectable").unwrap_or(false),
            )
        };
        let spans = prop_spans(props, "spans", ctx);
        let mut p = JsProps::new(props, ctx);
        TextView {
            text: p.val::<String>("text"),
            font_size: p.val::<f64>("fontSize"),
            color: p.val::<Color>("color"),
            spans,
            query_key: p.query_key("queryKey"),
            on_selection_change,
            selectable,
            max_lines: p.val::<u32>("maxLines"),
            overflow: p.val::<TextOverflow>("overflow"),
        }
    }
}
