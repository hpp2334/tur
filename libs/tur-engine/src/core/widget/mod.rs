use std::rc::Rc;

use boa_engine::Context;
use boa_gc::{Finalize, Trace};

use crate::core::element::ElementNodeId;

pub mod callback;
pub mod context;
pub mod val;

pub use callback::{
    callback_from_js, extract_callback_from_opts, make_mutation_callback, mutation_from_js,
    Callback, EventArg, Mutation, ReturnVal,
};
pub use context::WidgetCx;
pub use val::{val_from_js, PropValue, ReadableAtom, Val};

// ---------------------------------------------------------------------------
// Spec — the user's declaration of a widget. Pure Rust data (no JsValues).
//
// A Spec describes HOW to build a widget: its reactive props (Val<T>) and
// its children (Vec<Rc<dyn Spec>>).  `build()` instantiates the widget into
// the ElementTree.  Specs are immutable after creation; Condition/LazyList
// hold onto branch/range specs and rebuild subtrees on demand.
// ---------------------------------------------------------------------------

pub trait Spec: 'static {
    fn build(
        &self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        parent: ElementNodeId,
    ) -> ElementNodeId;
}

// ---------------------------------------------------------------------------
// SpecHandle — boa opaque wrapper so JS can hold and pass specs around.
//
// `unsafe_empty_trace` is safe because specs contain NO JsValues/JsObjects:
// all props are decoded to pure Rust types (Val<T> where T: PropValue) at
// factory time.  Children are `Rc<dyn Spec>` (pure Rust ownership).
// ---------------------------------------------------------------------------

#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct SpecHandle(pub Rc<dyn Spec>);

impl SpecHandle {
    pub fn new(spec: Rc<dyn Spec>) -> Self {
        SpecHandle(spec)
    }
}

/// Extract a spec from a JS value wrapping a `SpecHandle`.
pub fn extract_spec(value: &boa_engine::JsValue) -> Option<Rc<dyn Spec>> {
    let obj = value.as_object()?;
    obj.downcast_ref::<SpecHandle>().map(|h| h.0.clone())
}

// ---------------------------------------------------------------------------
// Effect — optional lifecycle hook for widgets that mutate the tree in
// response to reactive changes (Condition swaps branches, LazyList adjusts
// its visible range).  Default impl is a no-op so every widget type
// satisfies the bound without boilerplate.
// ---------------------------------------------------------------------------

pub trait Effect {
    fn effect(
        &mut self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<crate::core::reactive::AtomId>,
    ) {
        let _ = (cx, boa, dirties);
    }
}
