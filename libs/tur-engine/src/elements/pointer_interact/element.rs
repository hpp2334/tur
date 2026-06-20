use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use tur_shared::{HitTestBehavior, Offset};

use crate::core::edgy_event::{edgy_mutation_from_js, EdgyMutation, EventArg};
use crate::core::element::ElementNodeId;
use crate::core::elements::{ComposedGestureEvent, ElementOnGesture, ElementOnGestureContext};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::widget::{
    extract_component, val_from_js, Effect, PropValue, Component, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// PointerInteractComponent — the user's declaration. Pure Rust, no JsValues.
//
// Callbacks are mutation atoms typed as `EdgyMutation<E>`. The JS bridge
// wraps user callbacks as mutation atoms and passes the `AtomHandle` as the
// prop value. At event time the gesture handler resolves these and pushes
// invocations onto the pending-mutation queue.
//
// Enter/exit hover callbacks live on `MouseRegion` (which also manages the
// OS cursor). PointerInteract is gesture-only: click + drag.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PointerInteractComponent {
    pub behavior: Option<Val<HitTestBehavior>>,
    pub on_click: Option<EdgyMutation<PointerInteractEvent>>,
    pub on_pointer_down: Option<EdgyMutation<PointerInteractEvent>>,
    pub on_pointer_move: Option<EdgyMutation<PointerInteractEvent>>,
    pub on_pointer_up: Option<EdgyMutation<PointerInteractEvent>>,
    pub child: Option<Rc<dyn Component>>,
}

impl Component for PointerInteractComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let behavior = self
            .behavior
            .as_ref()
            .and_then(|v| cx.read_val(v, boa))
            .unwrap_or_default();

        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::with_gesture(PointerInteractElement {
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
// PointerInteractElement — the built element. Stores spec + eagerly-resolved
// behavior (read by the gesture handler at event time where no store/Context
// is available).
// ---------------------------------------------------------------------------

pub struct PointerInteractElement {
    pub component: PointerInteractComponent,
    behavior: HitTestBehavior,
}

impl PointerInteractElement {
    pub fn has_on_click(&self) -> bool {
        self.component.on_click.is_some()
    }

    pub fn has_gesture_callbacks(&self) -> bool {
        self.component.on_pointer_down.is_some()
            || self.component.on_pointer_move.is_some()
            || self.component.on_pointer_up.is_some()
    }

    pub fn is_click_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque && self.component.on_click.is_some()
    }
}

impl Effect for PointerInteractElement {}

impl ElementTrace for PointerInteractElement {}

impl ElementOnGesture for PointerInteractElement {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) {
        let (mutation, payload) = match event {
            ComposedGestureEvent::PointerDown { local, global } => {
                let m = self.component.on_pointer_down;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
            ComposedGestureEvent::PointerMove { local, global } => {
                let m = self.component.on_pointer_move;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
            ComposedGestureEvent::PointerUp { local, global } => {
                let m = self.component.on_pointer_up;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
        };
        if let Some(m) = mutation {
            cx.push_event(m, payload);
        }
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

impl PointerInteractComponent {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        PointerInteractComponent {
            behavior: prop_val::<HitTestBehavior>(props, "behavior", ctx),
            on_click: prop_mutation::<PointerInteractEvent>(props, "onClick", ctx),
            on_pointer_down: prop_mutation::<PointerInteractEvent>(props, "onPointerDown", ctx),
            on_pointer_move: prop_mutation::<PointerInteractEvent>(props, "onPointerMove", ctx),
            on_pointer_up: prop_mutation::<PointerInteractEvent>(props, "onPointerUp", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// PointerInteractEvent — JS callback argument for click / drag events.
// Carries both local (element-relative) and global (canvas-relative) coords.
// Serialises to a single JS object `{ local: {x, y}, global: {x, y} }`.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PointerInteractEvent {
    pub local: Offset,
    pub global: Offset,
}

impl EventArg for PointerInteractEvent {
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
