use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use tur_shared::{Alignment, BorderPosition, Brush, Color, Curve};

use crate::core::animation::host::ImplicitAnimationHost;
use crate::core::animation::props::AnimatedProp;
use crate::core::animation::event::AnimationEndEvent;
use crate::core::edgy_event::{extract_mutation_from_opts, EdgyMutation};
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::layout::ElementSubscribe;
use crate::core::view::{
    val_from_js, Lifecycle, PropValue, SharedViewCx, Val, View, ViewCx,
};

use crate::elements::container::ContainerPainting;

// ---------------------------------------------------------------------------
// AnimatedContainerView — the user's declaration. Pure Rust, no JsValues.
//
// Superset of ContainerView: adds `duration`, `curve`, optional `onEnd`.
// Each animatable prop is a `Val<T>` (static or reactive) — exactly like
// Container. The element animates from the previously-displayed value to the
// newly-resolved target over `duration` whenever the target changes
// (Flutter's `AnimatedContainer` / `ImplicitlyAnimatedWidget` semantics).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AnimatedContainerView {
    // animatable props
    pub width: Option<Val<f64>>,
    pub height: Option<Val<f64>>,
    pub padding: Option<Val<f64>>,
    pub color: Option<Val<Brush>>,
    pub border_color: Option<Val<Color>>,
    pub border_width: Option<Val<f64>>,
    pub border_radius: Option<Val<f64>>,
    pub shadow_color: Option<Val<Color>>,
    pub shadow_blur: Option<Val<f64>>,
    // non-animatable pass-through
    pub alignment: Option<Val<Alignment>>,
    pub border_position: Option<Val<BorderPosition>>,
    pub shadow_offset: Option<(f64, f64)>,
    // animation config
    pub duration_ms: u64,
    pub curve: Curve,
    pub on_end: Option<EdgyMutation<AnimationEndEvent>>,
    pub query_key: Option<Vec<String>>,
    pub children: Vec<Rc<dyn View>>,
}

impl View for AnimatedContainerView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        // Seed each animatable prop at its current target so the first layout
        // paints the targets (no animation). The Effect phase later handles
        // registration + retarget when targets change. Seeding here is needed
        // because Effect doesn't run on mount (no atom is dirty yet).
        let p_width = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.width.as_ref(), boa));
        let p_height = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.height.as_ref(), boa));
        let p_padding = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.padding.as_ref(), boa));
        let p_color = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.color.as_ref(), boa));
        let p_border_color = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.border_color.as_ref(), boa));
        let p_border_width = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.border_width.as_ref(), boa));
        let p_border_radius = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.border_radius.as_ref(), boa));
        let p_shadow_color = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.shadow_color.as_ref(), boa));
        let p_shadow_blur = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.shadow_blur.as_ref(), boa));
        let on_end = self.on_end;
        cx.insert_node(
            id,
            AnyElement::new(AnimatedContainerElement {
                view: self.clone(),
                painting: ContainerPainting::default(),
                host: ImplicitAnimationHost::new(on_end),
                element_id: id,
                p_width,
                p_height,
                p_padding,
                p_color,
                p_border_color,
                p_border_width,
                p_border_radius,
                p_shadow_color,
                p_shadow_blur,
            }),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        for child_spec in &self.children {
            let _child_id = child_spec.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// AnimatedContainerElement — the built element. Holds per-prop animation
// state + an ImplicitAnimationHost (the shared timeline handle). Layout
// resolves targets, lerps at the host's eased `t` into `painting`, then
// delegates sizing to the same math ContainerElement uses.
// ---------------------------------------------------------------------------

pub struct AnimatedContainerElement {
    pub view: AnimatedContainerView,
    pub painting: ContainerPainting,
    pub host: ImplicitAnimationHost,
    pub element_id: ElementNodeId,
    // per-prop animation state
    pub p_width: AnimatedProp<f64>,
    pub p_height: AnimatedProp<f64>,
    pub p_padding: AnimatedProp<f64>,
    pub p_color: AnimatedProp<Brush>,
    pub p_border_color: AnimatedProp<Color>,
    pub p_border_width: AnimatedProp<f64>,
    pub p_border_radius: AnimatedProp<f64>,
    pub p_shadow_color: AnimatedProp<Color>,
    pub p_shadow_blur: AnimatedProp<f64>,
}

impl AnimatedContainerElement {
    pub fn new(view: AnimatedContainerView, element_id: ElementNodeId) -> Self {
        let on_end = view.on_end;
        AnimatedContainerElement {
            view,
            painting: ContainerPainting::default(),
            host: ImplicitAnimationHost::new(on_end),
            element_id,
            p_width: AnimatedProp::new(),
            p_height: AnimatedProp::new(),
            p_padding: AnimatedProp::new(),
            p_color: AnimatedProp::new(),
            p_border_color: AnimatedProp::new(),
            p_border_width: AnimatedProp::new(),
            p_border_radius: AnimatedProp::new(),
            p_shadow_color: AnimatedProp::new(),
            p_shadow_blur: AnimatedProp::new(),
        }
    }
}// Brush lerp: SolidColor→SolidColor interpolates via Color::lerp; any
// gradient involvement snaps to `end` (gradient interpolation is out of
// scope for v1, matching Flutter where DecorationTween handles it specially).
pub(super) fn lerp_brush(a: &Brush, b: &Brush, t: f64) -> Brush {
    match (a, b) {
        (Brush::SolidColor(ca), Brush::SolidColor(cb)) => {
            Brush::SolidColor(Color::lerp(*ca, *cb, t))
        }
        _ => b.clone(),
    }
}

fn lerp_f64(a: &f64, b: &f64, t: f64) -> f64 {
    a + (b - a) * t
}

impl Lifecycle for AnimatedContainerElement {
    fn on_updated(
        &mut self,
        cx: &mut SharedViewCx,
        boa: &mut Context,
    ) {
        // Lazy one-time driver registration. The host owns the on_end callback
        // and hands it to the manager on first registration.
        self.host
            .ensure_registered(cx, self.element_id, self.view.duration_ms, self.view.curve);

        let eased_t = self.host.eased_t();
        let mut changed = false;

        // Feed each animatable prop its freshly-resolved target. On a change,
        // rebase `begin` to the currently-displayed value (captured BEFORE the
        // target update, so it reflects the old tween range) so the retargeted
        // timeline starts from the visible value (continuity).
        macro_rules! feed {
            ($prop:ident, $src:expr, $lerp:expr) => {
                let target = $src.and_then(|v| cx.read_val(v, boa));
                let displayed_before = self.$prop.evaluate(eased_t, $lerp);
                let (c, _first) = self.$prop.update_target(target);
                if c {
                    self.$prop.rebase_begin(displayed_before);
                    changed = true;
                }
            };
        }
        feed!(p_width, self.view.width.as_ref(), lerp_f64);
        feed!(p_height, self.view.height.as_ref(), lerp_f64);
        feed!(p_padding, self.view.padding.as_ref(), lerp_f64);
        feed!(p_border_width, self.view.border_width.as_ref(), lerp_f64);
        feed!(p_border_radius, self.view.border_radius.as_ref(), lerp_f64);
        feed!(p_shadow_blur, self.view.shadow_blur.as_ref(), lerp_f64);
        feed!(p_color, self.view.color.as_ref(), |a: &Brush, b: &Brush, t| lerp_brush(a, b, t));
        feed!(p_border_color, self.view.border_color.as_ref(), |a: &Color, b: &Color, t| Color::lerp(*a, *b, t));
        feed!(p_shadow_color, self.view.shadow_color.as_ref(), |a: &Color, b: &Color, t| Color::lerp(*a, *b, t));

        if changed {
            self.host.retarget(cx, self.element_id);
        }
    }
}

impl ElementSubscribe for AnimatedContainerElement {
    fn subscribe(&self, cx: &mut crate::core::layout::SubscribeCx) {
        // Subscribe to every animatable prop's atom so a target change marks
        // this node dirty → Effect runs → retarget fires.
        let c = &self.view;
        if let Some(v) = c.width.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.height.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.padding.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.color.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.border_color.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.border_width.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.border_radius.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.shadow_color.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.shadow_blur.as_ref() { cx.subscribe_val(v); }
        // Non-animatable but still reactive (affect layout/paint).
        if let Some(v) = c.alignment.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.border_position.as_ref() { cx.subscribe_val(v); }
    }
}

impl ElementTrace for AnimatedContainerElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(w) = self.p_width.target() {
            parts.push(format!("width={w}"));
        }
        if let Some(h) = self.p_height.target() {
            parts.push(format!("height={h}"));
        }
        parts.join(" ")
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let mut p = Vec::new();
        if let Some(v) = self.p_width.target() {
            p.push(("width", TraceValue::Num(*v)));
        }
        if let Some(v) = self.p_height.target() {
            p.push(("height", TraceValue::Num(*v)));
        }
        if let Some(v) = self.p_padding.target() {
            p.push(("padding", TraceValue::Num(*v)));
        }
        if let Some(v) = self.p_border_width.target() {
            p.push(("borderWidth", TraceValue::Num(*v)));
        }
        if let Some(v) = self.p_border_radius.target() {
            p.push(("borderRadius", TraceValue::Num(*v)));
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

fn prop_val<T: PropValue>(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

fn prop_offset(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<(f64, f64)> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    let obj = v.as_object()?;
    let arr = JsArray::from_object(obj.clone()).ok()?;
    let x = arr.at(0, ctx).ok()?.as_number()?;
    let y = arr.at(1, ctx).ok()?.as_number()?;
    Some((x, y))
}

fn prop_query_key(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<Vec<String>> {
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

fn prop_children(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Vec<Rc<dyn View>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    use crate::core::view::extract_view;
    let Ok(v) = props.get(js_string!(key), ctx) else {
        return Vec::new();
    };
    let Some(obj) = v.as_object() else {
        return Vec::new();
    };
    let Ok(arr) = JsArray::from_object(obj.clone()) else {
        return Vec::new();
    };
    let len = arr.length(ctx).unwrap_or(0);
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
        if let Ok(item) = arr.at(i as i64, ctx) {
            if let Some(spec) = extract_view(&item) {
                out.push(spec);
            }
        }
    }
    out
}

impl AnimatedContainerView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let duration_ms = prop_val::<f64>(props, "duration", ctx)
            .and_then(|v| v.as_static().copied())
            .map(|ms| ms.max(0.0) as u64)
            .unwrap_or(300);
        let curve = prop_val::<String>(props, "curve", ctx)
            .and_then(|v| v.as_static().cloned())
            .and_then(|s| s.parse::<Curve>().ok())
            .unwrap_or(Curve::Linear);
        let on_end = extract_mutation_from_opts(props, "onEnd", ctx);

        // Fold duration + curve into the query key so changing them forces a
        // clean remount (duration/curve are treated as fixed per element
        // lifetime by the driver).
        let mut query_key = prop_query_key(props, "queryKey", ctx).unwrap_or_default();
        query_key.push(format!("dur={}", duration_ms));
        query_key.push(format!("curve={:?}", curve));

        AnimatedContainerView {
            width: prop_val::<f64>(props, "width", ctx),
            height: prop_val::<f64>(props, "height", ctx),
            padding: prop_val::<f64>(props, "padding", ctx),
            color: prop_val::<Brush>(props, "color", ctx),
            border_color: prop_val::<Color>(props, "borderColor", ctx),
            border_width: prop_val::<f64>(props, "borderWidth", ctx),
            border_radius: prop_val::<f64>(props, "borderRadius", ctx),
            shadow_color: prop_val::<Color>(props, "shadowColor", ctx),
            shadow_blur: prop_val::<f64>(props, "shadowBlur", ctx),
            alignment: prop_val::<Alignment>(props, "alignment", ctx),
            border_position: prop_val::<BorderPosition>(props, "borderPosition", ctx),
            shadow_offset: prop_offset(props, "shadowOffset", ctx),
            duration_ms,
            curve,
            on_end,
            query_key: if query_key.is_empty() { None } else { Some(query_key) },
            children: prop_children(props, "children", ctx),
        }
    }
}
