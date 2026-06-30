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
// AnimatedOpacityView — implicit-animation Opacity (Flutter's
// `AnimatedOpacity`). Animates `value` (alpha 0..1) from the previous value
// to the new target over `duration` whenever it changes.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AnimatedOpacityView {
    pub value: Option<Val<f32>>,
    pub duration_ms: u64,
    pub curve: Curve,
    pub on_end: Option<EdgyMutation<AnimationEndEvent>>,
    pub query_key: Option<Vec<String>>,
    pub child: Option<Rc<dyn View>>,
}

impl View for AnimatedOpacityView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        let on_end = self.on_end;
        // Seed the opacity prop at its current target so the first layout
        // paints it without animation (Effect doesn't run on mount).
        let p_value = AnimatedProp::seeded(crate::core::view::read_val_opt(cx, self.value.as_ref(), boa));
        cx.insert_node(
            id,
            AnyElement::new(AnimatedOpacityElement {
                view: self.clone(),
                painting: 1.0,
                host: ImplicitAnimationHost::new(on_end),
                element_id: id,
                p_value,
            }),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

pub struct AnimatedOpacityElement {
    pub view: AnimatedOpacityView,
    pub painting: f32,
    pub host: ImplicitAnimationHost,
    pub element_id: ElementNodeId,
    pub p_value: AnimatedProp<f32>,
}

pub(super) fn lerp_f32(a: &f32, b: &f32, t: f64) -> f32 {
    a + (b - a) * (t as f32)
}

impl Lifecycle for AnimatedOpacityElement {
    fn on_updated(
        &mut self,
        cx: &mut SharedViewCx,
        boa: &mut Context,
    ) {
        self.host
            .ensure_registered(cx, self.element_id, self.view.duration_ms, self.view.curve);
        let eased_t = self.host.eased_t();
        let target = self
            .view
            .value
            .as_ref()
            .and_then(|v| cx.read_val(v, boa));
        let displayed_before = self.p_value.evaluate(eased_t, lerp_f32);
        let (changed, _first) = self.p_value.update_target(target);
        if changed {
            self.p_value.rebase_begin(displayed_before);
            self.host.retarget(cx, self.element_id);
        }
    }
}

impl ElementSubscribe for AnimatedOpacityElement {
    fn subscribe(&self, cx: &mut crate::core::layout::SubscribeCx) {
        if let Some(v) = self.view.value.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for AnimatedOpacityElement {
    fn trace_label(&self) -> String {
        self.p_value
            .target()
            .map(|v| format!("opacity={v}"))
            .unwrap_or_default()
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        self.p_value
            .target()
            .map(|v| vec![("opacity", TraceValue::Num(*v as f64))])
            .unwrap_or_default()
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

impl AnimatedOpacityView {
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

        let mut query_key = prop_query_key(props, "queryKey", ctx).unwrap_or_default();
        query_key.push(format!("dur={}", duration_ms));
        query_key.push(format!("curve={:?}", curve));

        let child = {
            use boa_engine::js_string;
            let v = props.get(js_string!("child"), ctx).ok();
            v.and_then(|val| extract_view(&val))
        };

        AnimatedOpacityView {
            value: prop_val::<f32>(props, "value", ctx),
            duration_ms,
            curve,
            on_end,
            query_key: if query_key.is_empty() { None } else { Some(query_key) },
            child,
        }
    }
}
