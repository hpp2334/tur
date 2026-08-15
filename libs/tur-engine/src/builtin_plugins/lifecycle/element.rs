use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsValue};

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
    pub(crate) factory: JsFunction,
}

impl View for LifecycleView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let descriptor = match self.factory.call(&JsValue::undefined(), &[], boa) {
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

        let id: ElementNodeId = cx.alloc_node().as_element_id();
        cx.insert_node(
            id,
            AnyElement::new(LifecycleElement {
                on_mounted,
                before_destroy,
            }),
            boa,
        );
        if let Some(child) = element_view {
            child.build(cx, boa, id.into());
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
    fn on_mounted(&mut self, cx: &mut SharedViewCx, _boa: &mut Context) {
        if let Some(m) = self.on_mounted {
            cx.mutation_queue().borrow_mut().push(m, ());
        }
    }

    fn before_destroy(&mut self, cx: &mut SharedViewCx, _boa: &mut Context) {
        if let Some(m) = self.before_destroy {
            cx.mutation_queue().borrow_mut().push(m, ());
        }
    }
}
