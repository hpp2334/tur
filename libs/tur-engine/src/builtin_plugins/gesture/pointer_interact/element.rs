use std::rc::Rc;

use crate::core::edgy::mutation::{MutationHandle, MutationPayload};
use crate::core::edgy::value::Value;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::elements::{
    ComposedGestureEvent, ElementOnGesture, ElementOnGestureContext, TraceValue,
};
use crate::core::layout::{HitTestBehavior, Offset};
use crate::core::view::{Lifecycle, Val, View, ViewCx, read_val};

// ---------------------------------------------------------------------------
// PointerInteractView — the user's declaration. Pure Rust.
//
// Callbacks are mutation atoms typed as `MutationHandle<E>` (the rut rows
// mint them via `build_mutate`). At event time the gesture handler resolves
// these and pushes invocations onto the pending-mutation queue.
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
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let behavior = self
            .behavior
            .as_ref()
            .and_then(|v| read_val(cx, v))
            .unwrap_or_default();

        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::with_gesture(PointerInteractElement {
                view: self.clone(),
                behavior,
            })
            .with_callbacks(),
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child) = &self.child {
            child.build(cx, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// PointerInteractElement — the built element. Stores spec + eagerly-resolved
// behavior (read by the gesture handler at event time where no store is
// available).
// ---------------------------------------------------------------------------

pub struct PointerInteractElement {
    pub view: PointerInteractView,
    behavior: HitTestBehavior,
}

impl PointerInteractElement {
    pub fn has_on_click(&self) -> bool {
        self.view.on_click.is_some()
    }

    /// The resolved hit-test behavior (default `Opaque`). Drives both the
    /// element-tree hit walk (`hit_test_self`) and the gesture handler's
    /// claim probing.
    pub fn behavior(&self) -> HitTestBehavior {
        self.behavior
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
    fn on_gesture_event(&mut self, cx: &mut ElementOnGestureContext, event: &ComposedGestureEvent) {
        let (mutation, payload) = match event {
            ComposedGestureEvent::PointerDown { local, global, .. } => {
                let m = self.view.on_pointer_down;
                let ev = PointerInteractEvent {
                    local: *local,
                    global: *global,
                };
                (m, ev)
            }
            ComposedGestureEvent::PointerDoubleDown { local, global, .. }
            | ComposedGestureEvent::PointerTripleDown { local, global, .. } => {
                let m = self.view.on_pointer_down;
                let ev = PointerInteractEvent {
                    local: *local,
                    global: *global,
                };
                (m, ev)
            }
            ComposedGestureEvent::PointerMove { local, global, .. } => {
                let m = self.view.on_pointer_move;
                let ev = PointerInteractEvent {
                    local: *local,
                    global: *global,
                };
                (m, ev)
            }
            ComposedGestureEvent::PointerUp { local, global, .. } => {
                let m = self.view.on_pointer_up;
                let ev = PointerInteractEvent {
                    local: *local,
                    global: *global,
                };
                (m, ev)
            }
            ComposedGestureEvent::Click { local, global, .. } => {
                let m = self.view.on_click;
                let ev = PointerInteractEvent {
                    local: *local,
                    global: *global,
                };
                (m, ev)
            }
            ComposedGestureEvent::ContextMenu { local, global, .. } => {
                let m = self.view.on_context_menu;
                let ev = PointerInteractEvent {
                    local: *local,
                    global: *global,
                };
                (m, ev)
            }
        };
        if let Some(m) = mutation {
            cx.push_event(m, payload);
        }
    }
}

// ---------------------------------------------------------------------------
// PointerInteractEvent — callback argument for click / drag events.
// Carries both local (element-relative) and global (canvas-relative) coords.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PointerInteractEvent {
    pub local: Offset,
    pub global: Offset,
}

impl MutationPayload for PointerInteractEvent {
    /// Native crossing (the rut rail): `[local.x, local.y, global.x,
    /// global.y]` — realm-free.
    fn to_value_args(&self) -> Vec<Value> {
        vec![
            Value::Num(self.local.x),
            Value::Num(self.local.y),
            Value::Num(self.global.x),
            Value::Num(self.global.y),
        ]
    }
}
