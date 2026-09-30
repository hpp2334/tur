use boa_engine::JsValue;
use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;

use crate::core::edgy::mutation::MutationHandle;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::js_runtime::JsProps;
use crate::core::layout::ElementSubscribe;
use crate::core::view::{Lifecycle, SharedViewCx, View, ViewCx, extract_view};

// ---------------------------------------------------------------------------
// LifecycleView — wraps a JS factory `() => { element, onMounted$?, beforeDestroy$? }`.
//
// The factory is invoked once at build time. It returns the child `element`
// plus optional `onMounted$` / `beforeDestroy$` mutation callbacks, which fire
// at the element's mount / destroy lifecycle points (driven centrally by the
// flush loop). The wrapper is a transparent pass-through for layout / paint.
// ---------------------------------------------------------------------------

pub struct LifecycleView {
    pub(crate) factory: LifecycleFactory,
}

/// The descriptor source: a JS thunk (the JS rail) or a pre-built rut
/// descriptor (child + intent mutations — the C7 rows).
pub(crate) enum LifecycleFactory {
    Js(JsFunction),
    Rut {
        child: Rc<dyn View>,
        on_mounted: Option<MutationHandle<()>>,
        before_destroy: Option<MutationHandle<()>>,
    },
}

impl View for LifecycleView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        // The rut descriptor needs no realm — resolve it first.
        if let LifecycleFactory::Rut {
            child,
            on_mounted,
            before_destroy,
        } = &self.factory
        {
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
            return id.into();
        }
        // The descriptor factory is a JS thunk — it can only exist on an
        // instance with a realm. A realm-free build cannot reach this arm.
        let LifecycleFactory::Js(factory) = &self.factory else {
            unreachable!("rut arm returned above");
        };
        let Some(boa) = cx.realm() else {
            tracing::warn!("lifecycleView::build skipped: no JS realm (JS factory)");
            return parent;
        };
        let descriptor = match factory.call(&JsValue::undefined(), &[], boa) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("lifecycleView factory error: {e}");
                return parent;
            }
        };
        let Some(obj) = descriptor.as_object() else {
            tracing::error!("lifecycleView factory must return an object");
            return parent;
        };

        let (element_view, on_mounted, before_destroy) = {
            let mut p = JsProps::new(&obj, boa);
            let element_view = {
                let v = p.raw_opt("element").unwrap_or(JsValue::undefined());
                extract_view(&v)
            };
            (
                element_view,
                p.mutation::<()>("onMounted$"),
                p.mutation::<()>("beforeDestroy$"),
            )
        };

        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(LifecycleElement {
                on_mounted,
                before_destroy,
            }),
        );
        if let Some(child) = element_view {
            child.build(cx, id.into());
        }
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
