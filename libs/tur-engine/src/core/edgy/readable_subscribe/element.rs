use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::js_runtime::JsProps;
use crate::core::edgy::mutation::MutationHandle;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::edgy::reactive::AnyReadable;
use crate::core::view::{Lifecycle, SharedViewCx, View, ViewCx};

// ---------------------------------------------------------------------------
// ReadableSubscribeView — subscribes to a list of readable atoms and fires an
// `onUpdate$` mutation after layout whenever any of them is dirtied.
//
// `child` is required (the wrapper is a transparent pass-through). This is the
// sole consumer of `Lifecycle::on_updated`.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ReadableSubscribeView {
    readables: Vec<AnyReadable>,
    on_update: Option<MutationHandle<()>>,
    child: Option<Rc<dyn View>>,
}

impl View for ReadableSubscribeView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(ReadableSubscribeElement {
                readables: self.readables.clone(),
                on_update: self.on_update,
            }),
            boa,
        );
        if let Some(child) = &self.child {
            child.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

pub struct ReadableSubscribeElement {
    readables: Vec<AnyReadable>,
    on_update: Option<MutationHandle<()>>,
}

impl ElementSubscribe for ReadableSubscribeElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        for atom in &self.readables {
            cx.subscribe_readable(*atom);
        }
    }
}

impl ElementTrace for ReadableSubscribeElement {
    fn trace_label(&self) -> String {
        String::new()
    }
}

impl Lifecycle for ReadableSubscribeElement {
    fn on_updated(&mut self, cx: &mut SharedViewCx, _boa: &mut Context) {
        if let Some(m) = self.on_update {
            cx.mutation_queue().borrow_mut().push(m, ());
        }
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

impl ReadableSubscribeView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        ReadableSubscribeView {
            readables: p.readables("readables"),
            on_update: p.mutation::<()>("onUpdate$"),
            child: p.child("child"),
        }
    }
}
