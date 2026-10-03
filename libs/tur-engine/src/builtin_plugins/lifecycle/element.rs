use std::rc::Rc;

use crate::core::edgy::mutation::{MutationHandle, MutationPayload};
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::layout::ElementSubscribe;
use crate::core::view::{Lifecycle, SharedViewCx, View, ViewCx};

// ---------------------------------------------------------------------------
// LifecycleView — wraps a pre-built child with mount/unmount intent
// callbacks.
//
// The child view is authored at start time (the rut rows' `el_lifecycle`);
// the `onMounted$` / `beforeDestroy$` intent mutations fire at the element's
// mount / destroy lifecycle points (driven centrally by the flush loop). The
// wrapper is a transparent pass-through for layout / paint.
// ---------------------------------------------------------------------------

pub struct LifecycleView {
    pub(crate) factory: LifecycleFactory,
}

/// The pre-built rut descriptor: child + intent mutations (the C7 rows).
pub(crate) enum LifecycleFactory {
    Rut {
        child: Rc<dyn View>,
        on_mounted: Option<MutationHandle<()>>,
        before_destroy: Option<MutationHandle<()>>,
    },
}

impl View for LifecycleView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let LifecycleFactory::Rut {
            child,
            on_mounted,
            before_destroy,
        } = &self.factory;
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(LifecycleElement {
                on_mounted: *on_mounted,
                before_destroy: *before_destroy,
            }),
        );
        child.build(cx, id.into());
        cx.link_child(parent, id.into());
        id.into()
    }
}

pub struct LifecycleElement {
    on_mounted: Option<MutationHandle<()>>,
    before_destroy: Option<MutationHandle<()>>,
}

impl ElementTrace for LifecycleElement {
    fn trace_label(&self) -> String {
        String::new()
    }
}

// No reactive deps; the default no-op subscribe satisfies the bound.
impl ElementSubscribe for LifecycleElement {}

impl Lifecycle for LifecycleElement {
    fn on_mounted(&mut self, cx: &mut SharedViewCx) {
        if let Some(m) = self.on_mounted {
            cx.mutation_queue().borrow_mut().push(m, ());
        }
    }

    fn before_destroy(&mut self, cx: &mut SharedViewCx) {
        if let Some(m) = self.before_destroy {
            cx.mutation_queue().borrow_mut().push(m, ());
        }
    }
}

// The lifecycle intents carry no payload (the callbacks read their data
// from closure captures) — the empty default args.
impl MutationPayload for () {}
