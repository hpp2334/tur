use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::Context;
use boa_gc::{Finalize, Trace};

use crate::core::element::ElementNodeId;
use crate::core::reactive::AtomId;

pub mod context;
pub mod val;

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

// ---------------------------------------------------------------------------
// make_mutation_callback — build a JS function that, when called, invokes the
// mutation atom identified by `atom_id` with whatever arguments the caller
// passes.  Used during `Spec::build` to turn `onClick` / `onPointerEnter` /
// etc. mutation atoms into the `JsFunction`s that elements store for later
// callback emission.
//
// The returned `JsFunction` is NOT GC-traced by the element it lives in (the
// element sits behind `AnyElement`'s `unsafe_empty_trace`).  This is safe for
// the same reason it was in the old model: mutation closures are held alive
// by the reactive `Store` (on `TurJsContext`) for the lifetime of the app,
// so the GC always sees them via that root.
// ---------------------------------------------------------------------------

pub fn make_mutation_callback(
    cx: &WidgetCx,
    boa: &mut Context,
    atom_id: AtomId,
) -> Option<JsFunction> {
    let store = cx.store();
    let store_ctx_obj = crate::core::reactive::build_store_context_object(boa, store.clone())
        .ok()
        .map(boa_engine::JsValue::from);
    let store_for_closure = store.clone();
    let callback = unsafe {
        boa_engine::native_function::NativeFunction::from_closure(move |_this, args, ctx| {
            let mut full_args = Vec::with_capacity(args.len() + 1);
            if let Some(ref obj) = store_ctx_obj {
                full_args.push(obj.clone());
            }
            full_args.extend_from_slice(args);
            let _ = store_for_closure.borrow().invoke_mutation(atom_id, &full_args, ctx)?;
            Ok(boa_engine::JsValue::undefined())
        })
    };
    let func = boa_engine::object::FunctionObjectBuilder::new(boa.realm(), callback).build();
    Some(func)
}
