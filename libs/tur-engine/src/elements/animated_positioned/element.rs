use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use tur_shared::Curve;

use crate::core::animation::event::AnimationEndEvent;
use crate::core::animation::host::ImplicitAnimationHost;
use crate::core::animation::props::AnimatedProp;
use crate::core::edgy_event::{extract_mutation_from_opts, EdgyMutation};
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::layout::ElementSubscribe;
use crate::core::view::{
    extract_view, val_from_js, Lifecycle, PropValue, SharedViewCx, Val, View, ViewCx,
};

// ---------------------------------------------------------------------------
// AnimatedPositionedView — implicit-animation Positioned (Flutter's
// `AnimatedPositioned`), for use inside a `Stack`. Animates left/top/right/
// bottom/width/height from their previous values to new targets over
// `duration` whenever any changes.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AnimatedPositionedView {
    pub left: Option<Val<f64>>,
    pub top: Option<Val<f64>>,
    pub right: Option<Val<f64>>,
    pub bottom: Option<Val<f64>>,
    pub width: Option<Val<f64>>,
    pub height: Option<Val<f64>>,
    pub duration_ms: u64,
    pub curve: Curve,
    pub on_end: Option<EdgyMutation<AnimationEndEvent>>,
    pub query_key: Option<Vec<String>>,
    pub child: Rc<dyn View>,
}

impl View for AnimatedPositionedView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        let on_end = self.on_end;
        // Seed each animatable prop at its current target so the first layout
        // paints the targets (Effect doesn't run on mount).
        let p_left = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.left.as_ref(), boa));
        let p_top = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.top.as_ref(), boa));
        let p_right = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.right.as_ref(), boa));
        let p_bottom = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.bottom.as_ref(), boa));
        let p_width = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.width.as_ref(), boa));
        let p_height = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.height.as_ref(), boa));
        cx.insert_node(
            id,
            AnyElement::new(AnimatedPositionedElement {
                view: self.clone(),
                host: ImplicitAnimationHost::new(on_end),
                element_id: id,
                p_left,
                p_top,
                p_right,
                p_bottom,
                p_width,
                p_height,
            }),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        let _child_id = self.child.build(cx, boa, id.into());
        cx.link_child(parent, id.into());
        id.into()
    }
}

pub struct AnimatedPositionedElement {
    pub view: AnimatedPositionedView,
    pub host: ImplicitAnimationHost,
    pub element_id: ElementNodeId,
    pub p_left: AnimatedProp<f64>,
    pub p_top: AnimatedProp<f64>,
    pub p_right: AnimatedProp<f64>,
    pub p_bottom: AnimatedProp<f64>,
    pub p_width: AnimatedProp<f64>,
    pub p_height: AnimatedProp<f64>,
}

pub(super) fn lerp_f64(a: &f64, b: &f64, t: f64) -> f64 {
    a + (b - a) * t
}

impl Lifecycle for AnimatedPositionedElement {
    fn on_updated(
        &mut self,
        cx: &mut SharedViewCx,
        boa: &mut Context,
    ) {
        self.host
            .ensure_registered(cx, self.element_id, self.view.duration_ms, self.view.curve);
        let eased_t = self.host.eased_t();
        let mut changed = false;
        macro_rules! feed {
            ($prop:ident, $src:expr) => {
                let target = $src.and_then(|v| cx.read_val(v, boa));
                let displayed_before = self.$prop.evaluate(eased_t, lerp_f64);
                let (c, _first) = self.$prop.update_target(target);
                if c {
                    self.$prop.rebase_begin(displayed_before);
                    changed = true;
                }
            };
        }
        feed!(p_left, self.view.left.as_ref());
        feed!(p_top, self.view.top.as_ref());
        feed!(p_right, self.view.right.as_ref());
        feed!(p_bottom, self.view.bottom.as_ref());
        feed!(p_width, self.view.width.as_ref());
        feed!(p_height, self.view.height.as_ref());
        if changed {
            self.host.retarget(cx, self.element_id);
        }
    }
}

impl ElementSubscribe for AnimatedPositionedElement {
    fn subscribe(&self, cx: &mut crate::core::layout::SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.left.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.top.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.right.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.bottom.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.width.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.height.as_ref() { cx.subscribe_val(v); }
    }
}

impl ElementTrace for AnimatedPositionedElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(v) = self.p_left.target() { parts.push(format!("left={v}")); }
        if let Some(v) = self.p_top.target() { parts.push(format!("top={v}")); }
        parts.join(" ")
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let mut p = Vec::new();
        for (key, prop) in [
            ("left", &self.p_left),
            ("top", &self.p_top),
            ("right", &self.p_right),
            ("bottom", &self.p_bottom),
            ("width", &self.p_width),
            ("height", &self.p_height),
        ] {
            if let Some(v) = prop.target() {
                p.push((key, TraceValue::Num(*v)));
            }
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

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

impl AnimatedPositionedView {
    /// Build from a JS props object. Returns `None` when the required `child`
    /// prop is missing (mirrors `PositionedView::from_js`).
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let duration_ms = prop_val::<f64>(props, "duration", ctx)
            .and_then(|v| v.as_static().copied())
            .map(|ms| ms.max(0.0) as u64)
            .unwrap_or(300);
        let curve = prop_val::<String>(props, "curve", ctx)
            .and_then(|v| v.as_static().cloned())
            .and_then(|s| s.parse::<Curve>().ok())
            .unwrap_or(Curve::Linear);
        let on_end = extract_mutation_from_opts(props, "onEnd", ctx);

        let mut query_key = prop_query_key(props, "queryKey", ctx).unwrap_or_default();
        query_key.push(format!("dur={}", duration_ms));
        query_key.push(format!("curve={:?}", curve));

        let child = {
            use boa_engine::js_string;
            let v = props.get(js_string!("child"), ctx).ok()?;
            extract_view(&v)?
        };

        Some(AnimatedPositionedView {
            left: prop_val::<f64>(props, "left", ctx),
            top: prop_val::<f64>(props, "top", ctx),
            right: prop_val::<f64>(props, "right", ctx),
            bottom: prop_val::<f64>(props, "bottom", ctx),
            width: prop_val::<f64>(props, "width", ctx),
            height: prop_val::<f64>(props, "height", ctx),
            duration_ms,
            curve,
            on_end,
            query_key: if query_key.is_empty() { None } else { Some(query_key) },
            child,
        })
    }
}
