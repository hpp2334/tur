use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use tur_shared::{HitTestBehavior, Offset};

use crate::core::edgy_event::{edgy_mutation_from_js, EdgyMutation, EventArg};
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{ComposedGestureEvent, ElementOnGesture, ElementOnGestureContext, TraceValue};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::view::{
    ViewCx,
    read_val,
    extract_view, val_from_js, Lifecycle, PropValue, View, Val,
};

// ---------------------------------------------------------------------------
// PointerInteractView — the user's declaration. Pure Rust, no JsValues.
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
pub struct PointerInteractView {
    pub behavior: Option<Val<HitTestBehavior>>,
    pub on_click: Option<EdgyMutation<PointerInteractEvent>>,
    pub on_pointer_down: Option<EdgyMutation<PointerInteractEvent>>,
    pub on_pointer_move: Option<EdgyMutation<PointerInteractEvent>>,
    pub on_pointer_up: Option<EdgyMutation<PointerInteractEvent>>,
    pub on_context_menu: Option<EdgyMutation<PointerInteractEvent>>,
    pub query_key: Option<Vec<String>>,
    pub child: Option<Rc<dyn View>>,
}

impl View for PointerInteractView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let behavior = self
            .behavior
            .as_ref()
            .and_then(|v| read_val(cx, v, boa))
            .unwrap_or_default();

        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::with_gesture(PointerInteractElement {
                view: self.clone(),
                behavior,
            })
            .with_callbacks(),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child) = &self.child {
            child.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// PointerInteractElement — the built element. Stores spec + eagerly-resolved
// behavior (read by the gesture handler at event time where no store/Context
// is available).
// ---------------------------------------------------------------------------

pub struct PointerInteractElement {
    pub view: PointerInteractView,
    behavior: HitTestBehavior,
}

impl PointerInteractElement {
    pub fn has_on_click(&self) -> bool {
        self.view.on_click.is_some()
    }

    pub fn has_gesture_callbacks(&self) -> bool {
        self.view.on_pointer_down.is_some()
            || self.view.on_pointer_move.is_some()
            || self.view.on_pointer_up.is_some()
    }

    pub fn is_click_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque && self.view.on_click.is_some()
    }
}

impl crate::core::layout::ElementSubscribe for PointerInteractElement {}

impl Lifecycle for PointerInteractElement {}

impl ElementTrace for PointerInteractElement {
    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        vec![("behavior", TraceValue::Str(format!("{:?}", self.behavior)))]
    }
}

impl ElementOnGesture for PointerInteractElement {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) {
        let (mutation, payload) = match event {
            ComposedGestureEvent::PointerDown { local, global, .. } => {
                let m = self.view.on_pointer_down;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
            // Multi-click variants also fire `on_pointer_down` — matches
            // the DOM convention where `mousedown` is dispatched on every
            // click of a multi-click sequence. (`dblclick` is a separate
            // event there; elements that care about double-click as a
            // distinct gesture implement `ElementOnGesture` directly.)
            ComposedGestureEvent::PointerDoubleDown { local, global, .. }
            | ComposedGestureEvent::PointerTripleDown { local, global, .. } => {
                let m = self.view.on_pointer_down;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
            ComposedGestureEvent::PointerMove { local, global } => {
                let m = self.view.on_pointer_move;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
            ComposedGestureEvent::PointerUp { local, global, .. } => {
                let m = self.view.on_pointer_up;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
            ComposedGestureEvent::ContextMenu { local, global } => {
                let m = self.view.on_context_menu;
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

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn View>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_view(&v)
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
        let s = arr.get(i, ctx).ok()?;
        if let Some(s) = s.as_string() {
            out.push(s.to_std_string_escaped());
        }
    }
    Some(out)
}

impl PointerInteractView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        PointerInteractView {
            behavior: prop_val::<HitTestBehavior>(props, "behavior", ctx),
            on_click: prop_mutation::<PointerInteractEvent>(props, "onClick", ctx),
            on_pointer_down: prop_mutation::<PointerInteractEvent>(props, "onPointerDown", ctx),
            on_pointer_move: prop_mutation::<PointerInteractEvent>(props, "onPointerMove", ctx),
            on_pointer_up: prop_mutation::<PointerInteractEvent>(props, "onPointerUp", ctx),
            on_context_menu: prop_mutation::<PointerInteractEvent>(props, "onContextMenu", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
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
