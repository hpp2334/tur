use std::rc::Rc;

use crate::core::edgy::mutation::{MutationHandle, MutationPayload};
use crate::core::edgy::value::Value;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::layout::{HitTestBehavior, Offset, SubscribeCx};
use crate::core::shell::Cursor;
use crate::core::view::{Lifecycle, Val, View, ViewCx, read_val};

// ---------------------------------------------------------------------------
// MouseRegionView — the user's declaration. Pure Rust.
//
// `cursor` is reactive (`Val<Cursor>`); it is resolved to a concrete `Cursor`
// during layout and read by the pointer-region handler at event time.
// `on_enter` / `on_exit` are mutation atoms invoked by the pointer-region
// handler when this region enters or leaves the hit-path.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MouseRegionView {
    pub(crate) behavior: Option<Val<HitTestBehavior>>,
    pub(crate) cursor: Option<Val<Cursor>>,
    pub on_enter: Option<MutationHandle<PointerRegionEvent>>,
    pub on_exit: Option<MutationHandle<PointerRegionEvent>>,
    pub(crate) child: Option<Rc<dyn View>>,
}

impl View for MouseRegionView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let behavior = self
            .behavior
            .as_ref()
            .and_then(|v| read_val(cx, v))
            .unwrap_or_default();

        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(MouseRegionElement {
                view: self.clone(),
                behavior,
                cursor: None,
            })
            .with_callbacks(),
        );
        if let Some(child) = &self.child {
            child.build(cx, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// MouseRegionElement — the built element. Stores spec + eagerly-resolved
// behavior (resolved at build) and the layout-resolved `cursor`. Both are
// read by the pointer-region handler at event time, where no store is
// available.
// ---------------------------------------------------------------------------

pub struct MouseRegionElement {
    pub view: MouseRegionView,
    pub(crate) behavior: HitTestBehavior,
    pub(crate) cursor: Option<Cursor>,
}

impl MouseRegionElement {
    pub fn has_region_callbacks(&self) -> bool {
        self.view.on_enter.is_some() || self.view.on_exit.is_some()
    }

    pub fn has_cursor(&self) -> bool {
        self.view.cursor.is_some()
    }

    /// The layout-resolved cursor for this region, if any.
    pub fn resolved_cursor(&self) -> Option<Cursor> {
        self.cursor
    }

    pub fn is_region_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque && self.has_region_callbacks()
    }
}

impl crate::core::layout::ElementSubscribe for MouseRegionElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        if let Some(v) = self.view.cursor.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl Lifecycle for MouseRegionElement {}

impl ElementTrace for MouseRegionElement {
    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let mut p = vec![("behavior", TraceValue::Str(format!("{:?}", self.behavior)))];
        if let Some(c) = self.view.cursor.as_ref().and_then(Val::as_static) {
            p.push(("cursor", TraceValue::Str(c.as_str().to_string())));
        }
        p
    }
}

// ---------------------------------------------------------------------------
// PointerRegionEvent — callback argument for `onEnter` / `onExit`.
// Carries both local (element-relative) and global (canvas-relative) coords.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PointerRegionEvent {
    pub local: Offset,
    pub global: Offset,
}

impl MutationPayload for PointerRegionEvent {
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
