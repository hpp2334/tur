use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::element::ElementNodeId;

pub mod context;
pub mod val;

pub use context::WidgetCx;
pub use val::{val_from_js, PropValue, Val};

// ---------------------------------------------------------------------------
// Component — the user's declaration of a widget.
//
// For native widgets this is pure Rust data (no JsValues): reactive props are
// `Val<T>` and children are `Vec<Rc<dyn Component>>`. `build()` instantiates the
// widget into the ElementTree. Components are immutable after creation.
// ---------------------------------------------------------------------------

pub trait Component: 'static {
    fn build(
        &self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        parent: ElementNodeId,
    ) -> ElementNodeId;
}

// ---------------------------------------------------------------------------
// ComponentFactory — produces a Component on demand.
//
// Used for branches whose concrete subtree is only determined at runtime
// (Condition/Switch branches). The factory is retained and `create()` is
// invoked when the branch is selected, and re-invoked on a branch swap.
// ---------------------------------------------------------------------------

pub trait ComponentFactory: 'static {
    fn create(&self, boa: &mut Context) -> Option<Rc<dyn Component>>;
}

/// Invoke a JS thunk `() => EdgyElement` and resolve the returned Component.
fn invoke_thunk(thunk: &JsFunction, boa: &mut Context) -> Option<Rc<dyn Component>> {
    let result = match thunk.call(&JsValue::undefined(), &[], boa) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("invoke_thunk JS error: {e}");
            return None;
        }
    };
    extract_component(&result)
}

// ---------------------------------------------------------------------------
// JsComponent — a user-defined component created via `component(fn)` in JS.
//
// It wraps the JS thunk `() => EdgyElement` and is itself a Component: `build`
// invokes the thunk, resolves the returned Component, and builds it under the
// parent. It allocates no node of its own (transparent pass-through), so the
// element-tree shape is identical to invoking the thunk eagerly.
// ---------------------------------------------------------------------------

pub struct JsComponent(pub JsFunction);

impl Component for JsComponent {
    fn build(
        &self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        parent: ElementNodeId,
    ) -> ElementNodeId {
        match invoke_thunk(&self.0, boa) {
            Some(inner) => inner.build(cx, boa, parent),
            None => parent,
        }
    }
}

// ---------------------------------------------------------------------------
// JsComponentFactory — a ComponentFactory backed by a JS thunk
// `() => EdgyElement`. Used for Condition/Switch branches: `create()` invokes
// the thunk and returns the produced Component.
// ---------------------------------------------------------------------------

pub struct JsComponentFactory(pub JsFunction);

impl ComponentFactory for JsComponentFactory {
    fn create(&self, boa: &mut Context) -> Option<Rc<dyn Component>> {
        invoke_thunk(&self.0, boa)
    }
}

// ---------------------------------------------------------------------------
// ComponentHandle — boa opaque wrapper so JS can hold and pass Components.
//
// `unsafe_empty_trace` note: native Components contain no JsValues (props are
// pure Rust). JsComponent/JsComponentFactory (and Each/LazyList builders) do
// hold a `JsFunction`, but in this boa fork a `JsObject`/`JsFunction` clone
// held in Rust is a strong GC root via refcounting, so the conservative
// `unsafe_empty_trace` cannot collect them prematurely (worst case: a leak).
// ---------------------------------------------------------------------------

#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct ComponentHandle(pub Rc<dyn Component>);

impl ComponentHandle {
    pub fn new(component: Rc<dyn Component>) -> Self {
        ComponentHandle(component)
    }
}

/// Extract a Component from a JS value wrapping a `ComponentHandle`.
pub fn extract_component(value: &JsValue) -> Option<Rc<dyn Component>> {
    let obj = value.as_object()?;
    obj.downcast_ref::<ComponentHandle>().map(|h| h.0.clone())
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
