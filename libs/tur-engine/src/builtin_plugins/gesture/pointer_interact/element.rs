use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use crate::core::layout::{HitTestBehavior, Offset};

use crate::core::js_runtime::JsProps;
use crate::core::edgy::mutation::{MutationHandle, IntoJsArgs};
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{ComposedGestureEvent, ElementOnGesture, ElementOnGestureContext, TraceValue};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::view::{ViewCx, read_val, Lifecycle, Val, View};

// ---------------------------------------------------------------------------
// PointerInteractView — the user's declaration. Pure Rust, no JsValues.
//
// Callbacks are mutation atoms typed as `MutationHandle<E>`. The JS bridge
// wraps user callbacks as mutation atoms and passes the `Mutation` handle as the
// prop value. At event time the gesture handler resolves these and pushes
// invocations onto the pending-mutation queue.
//
// Enter/exit hover callbacks live on `MouseRegion` (which also manages the
// OS cursor). PointerInteract is gesture-only: click + drag.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PointerInteractView {
    pub behavior: Option<Val<HitTestBehavior>>,
    pub on_click: Option<MutationHandle<PointerInteractEvent>>,
    pub on_pointer_down: Option<MutationHandle<PointerInteractEvent>>,
    pub on_pointer_move: Option<MutationHandle<PointerInteractEvent>>,
    pub on_pointer_up: Option<MutationHandle<PointerInteractEvent>>,
    pub on_context_menu: Option<MutationHandle<PointerInteractEvent>>,
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
    ) -> bool {
        let (mutation, payload) = match event {
            ComposedGestureEvent::PointerDown { local, global, .. } => {
                let m = self.view.on_pointer_down;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
            ComposedGestureEvent::PointerDoubleDown { local, global, .. }
            | ComposedGestureEvent::PointerTripleDown { local, global, .. } => {
                let m = self.view.on_pointer_down;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
            ComposedGestureEvent::PointerMove { local, global, .. } => {
                let m = self.view.on_pointer_move;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
            ComposedGestureEvent::PointerUp { local, global, .. } => {
                let m = self.view.on_pointer_up;
                let ev = PointerInteractEvent { local: *local, global: *global };
                (m, ev)
            }
            ComposedGestureEvent::Click { local, global } => {
                let m = self.view.on_click;
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
        true
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl PointerInteractView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        PointerInteractView {
            behavior: p.val::<HitTestBehavior>("behavior"),
            on_click: p.mutation::<PointerInteractEvent>("onClick"),
            on_pointer_down: p.mutation::<PointerInteractEvent>("onPointerDown"),
            on_pointer_move: p.mutation::<PointerInteractEvent>("onPointerMove"),
            on_pointer_up: p.mutation::<PointerInteractEvent>("onPointerUp"),
            on_context_menu: p.mutation::<PointerInteractEvent>("onContextMenu"),
            query_key: p.query_key("queryKey"),
            child: p.child("child"),
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

impl IntoJsArgs for PointerInteractEvent {
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
