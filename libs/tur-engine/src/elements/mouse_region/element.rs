use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use tur_shared::{HitTestBehavior, Offset};

use crate::core::edgy_event::{edgy_mutation_from_js, EdgyMutation, EventArg};
use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::widget::{
    extract_component, val_from_js, Effect, PropValue, Component, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// MouseRegionComponent — the user's declaration. Pure Rust, no JsValues.
//
// `cursor` is reactive (`Val<String>`); the resolved value is read on every
// pointer-region hit-path change. `on_enter` / `on_exit` are mutation atoms
// invoked by the pointer-region handler when this region enters or leaves
// the hit-path.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MouseRegionComponent {
    pub behavior: Option<Val<HitTestBehavior>>,
    pub cursor: Option<Val<String>>,
    pub on_enter: Option<EdgyMutation<PointerRegionEvent>>,
    pub on_exit: Option<EdgyMutation<PointerRegionEvent>>,
    pub child: Option<Rc<dyn Component>>,
}

impl Component for MouseRegionComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let behavior = self
            .behavior
            .as_ref()
            .and_then(|v| cx.read_val(v, boa))
            .unwrap_or_default();

        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::new(MouseRegionElement {
                component: self.clone(),
                behavior,
            })
            .with_callbacks(),
            boa,
        );
        if let Some(child) = &self.child {
            child.build(cx, boa, id);
        }
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// MouseRegionElement — the built element. Stores spec + eagerly-resolved
// behavior (read by the pointer-region handler at event time where no
// store/Context is available).
// ---------------------------------------------------------------------------

pub struct MouseRegionElement {
    pub component: MouseRegionComponent,
    behavior: HitTestBehavior,
}

impl MouseRegionElement {
    pub fn has_region_callbacks(&self) -> bool {
        self.component.on_enter.is_some() || self.component.on_exit.is_some()
    }

    pub fn has_cursor(&self) -> bool {
        self.component.cursor.is_some()
    }

    pub fn is_region_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque && self.has_region_callbacks()
    }
}

impl crate::core::layout::ElementSubscribe for MouseRegionElement {}

impl Effect for MouseRegionElement {}

impl ElementTrace for MouseRegionElement {
    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let mut p = vec![("behavior", TraceValue::Str(format!("{:?}", self.behavior)))];
        if let Some(v) = self.component.cursor.as_ref().and_then(Val::as_static) {
            p.push(("cursor", TraceValue::Str(v.clone())));
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

fn prop_mutation<E: EventArg>(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<EdgyMutation<E>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    edgy_mutation_from_js(&v)
}

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Component>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_component(&v)
}

impl MouseRegionComponent {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        MouseRegionComponent {
            behavior: prop_val::<HitTestBehavior>(props, "behavior", ctx),
            cursor: prop_val::<String>(props, "cursor", ctx),
            on_enter: prop_mutation::<PointerRegionEvent>(props, "onEnter", ctx),
            on_exit: prop_mutation::<PointerRegionEvent>(props, "onExit", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// PointerRegionEvent — JS callback argument for `onEnter` / `onExit`.
// Serialises to a single JS object `{ local: {x, y}, global: {x, y} }`.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PointerRegionEvent {
    pub local: Offset,
    pub global: Offset,
}

impl EventArg for PointerRegionEvent {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue> {
        use boa_engine::js_string;
        use boa_engine::object::JsObject;

        fn make_point(ctx: &mut Context, x: f64, y: f64) -> JsObject {
            let obj = JsObject::with_object_proto(ctx.intrinsics());
            let _ = obj.create_data_property(js_string!("x"), JsValue::from(x), ctx);
            let _ = obj.create_data_property(js_string!("y"), JsValue::from(y), ctx);
            obj
        }
        fn make_event(ctx: &mut Context, local: JsObject, global: JsObject) -> JsObject {
            let obj = JsObject::with_object_proto(ctx.intrinsics());
            let _ = obj.create_data_property(js_string!("local"), JsValue::from(local), ctx);
            let _ = obj.create_data_property(js_string!("global"), JsValue::from(global), ctx);
            obj
        }

        let local = make_point(ctx, self.local.x, self.local.y);
        let global = make_point(ctx, self.global.x, self.global.y);
        let event = make_event(ctx, local, global);
        vec![JsValue::from(event)]
    }
}
