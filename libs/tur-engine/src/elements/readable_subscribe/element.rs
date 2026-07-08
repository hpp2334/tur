use std::rc::Rc;

use boa_engine::object::builtins::JsArray;
use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::edgy_event::{edgy_mutation_from_js, EdgyMutation};
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::reactive::{AnyReadable, FromBoaJsValue};
use crate::core::view::{Lifecycle, SharedViewCx, View, ViewCx, extract_view};

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
    on_update: Option<EdgyMutation<()>>,
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
    on_update: Option<EdgyMutation<()>>,
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

fn prop_mutation(props: &JsObject, key: &str, ctx: &mut Context) -> Option<EdgyMutation<()>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    edgy_mutation_from_js(&v)
}

fn prop_readables(props: &JsObject, key: &str, ctx: &mut Context) -> Vec<AnyReadable> {
    use boa_engine::js_string;
    let Ok(v) = props.get(js_string!(key), ctx) else {
        return Vec::new();
    };
    let Some(obj) = v.as_object() else {
        return Vec::new();
    };
    let Ok(arr) = JsArray::from_object(obj.clone()) else {
        return Vec::new();
    };
    let Ok(len) = arr.length(ctx) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
        if let Ok(item) = arr.at(i as i64, ctx) {
            if let Some(readable) = AnyReadable::from_js(&item) {
                out.push(readable);
            }
        }
    }
    out
}

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn View>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_view(&v)
}

impl ReadableSubscribeView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        ReadableSubscribeView {
            readables: prop_readables(props, "readables", ctx),
            on_update: prop_mutation(props, "onUpdate$", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}
