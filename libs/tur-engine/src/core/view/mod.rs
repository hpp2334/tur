use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::element::NodeId;

pub mod build_cx;
pub mod context;
pub mod val;

pub use build_cx::{read_atom_raw, read_val, read_val_opt, ViewCx};
pub use context::SharedViewCx;
pub use crate::core::js_value::{FromJs, IntoJs};
pub use val::{val_from_js, Val};

// ---------------------------------------------------------------------------
// View — the user's declaration of a view.
//
// For native views this is pure Rust data (no JsValues): reactive props are
// `Val<T>` and children are `Vec<Rc<dyn View>>`. `build()` instantiates the
// view into the node tree. Views are immutable after creation.
//
// `build` takes `&mut dyn ViewCx` so the trait stays object-safe (`dyn View`
// is used by `ViewHandle` / builders) while still accepting either a `SharedViewCx`
// (normal builds) or a layout-backed `ViewCx` impl (build-during-layout).
// ---------------------------------------------------------------------------

pub trait View: 'static {
    fn build(
        &self,
        cx: &mut dyn ViewCx,
        boa: &mut Context,
        parent: NodeId,
    ) -> NodeId;
}

// ---------------------------------------------------------------------------
// ViewFactory — produces a View on demand.
//
// Used for branches whose concrete subtree is only determined at runtime
// (Condition/Switch branches). The factory is retained and `create()` is
// invoked when the branch is selected, and re-invoked on a branch swap.
// ---------------------------------------------------------------------------

pub trait ViewFactory: 'static {
    fn create(&self, boa: &mut Context) -> Option<Rc<dyn View>>;
}

/// Invoke a JS thunk `() => Element` and resolve the returned View.
fn invoke_thunk(thunk: &JsFunction, boa: &mut Context) -> Option<Rc<dyn View>> {
    let result = match thunk.call(&JsValue::undefined(), &[], boa) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("invoke_thunk JS error: {e}");
            return None;
        }
    };
    extract_view(&result)
}

// ---------------------------------------------------------------------------
// JsView — a user-defined view created via `view(fn)` in JS.
//
// It wraps the JS thunk `() => Element` and is itself a View: `build`
// invokes the thunk, resolves the returned View, and builds it under the
// parent. It allocates no node of its own (transparent pass-through), so the
// element-tree shape is identical to invoking the thunk eagerly.
// ---------------------------------------------------------------------------

pub struct JsView(pub JsFunction);

impl View for JsView {
    fn build(
        &self,
        cx: &mut dyn ViewCx,
        boa: &mut Context,
        parent: NodeId,
    ) -> NodeId {
        match invoke_thunk(&self.0, boa) {
            Some(inner) => inner.build(cx, boa, parent),
            None => parent,
        }
    }
}

// ---------------------------------------------------------------------------
// JsViewFactory — a ViewFactory backed by a JS thunk
// `() => Element`. Used for Condition/Switch branches: `create()` invokes
// the thunk and returns the produced View.
// ---------------------------------------------------------------------------

pub struct JsViewFactory(pub JsFunction);

impl ViewFactory for JsViewFactory {
    fn create(&self, boa: &mut Context) -> Option<Rc<dyn View>> {
        invoke_thunk(&self.0, boa)
    }
}

// ---------------------------------------------------------------------------
// ViewHandle — boa opaque wrapper so JS can hold and pass Views.
//
// `unsafe_empty_trace` note: native Views contain no JsValues (props are
// pure Rust). JsView/JsViewFactory (and Each/LazyList builders) do
// hold a `JsFunction`, but in this boa fork a `JsObject`/`JsFunction` clone
// held in Rust is a strong GC root via refcounting, so the conservative
// `unsafe_empty_trace` cannot collect them prematurely (worst case: a leak).
// ---------------------------------------------------------------------------

#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct ViewHandle(pub Rc<dyn View>);

impl ViewHandle {
    pub fn new(view: Rc<dyn View>) -> Self {
        ViewHandle(view)
    }
}

/// Extract a View from a JS value wrapping a `ViewHandle`.
pub fn extract_view(value: &JsValue) -> Option<Rc<dyn View>> {
    let obj = value.as_object()?;
    obj.downcast_ref::<ViewHandle>().map(|h| h.0.clone())
}

// ---------------------------------------------------------------------------
// Lifecycle — optional element lifecycle hooks. All default to no-op
// so every element type satisfies the bound without boilerplate.
//
//   * `on_mounted`       — fired once, right after the element is inserted
//                          into the tree (in `SharedViewCx::insert_node`).
//   * `on_updated`       — fired after layout, for each element whose
//                          subscribed atoms were dirtied during the reactive
//                          flush (driven by the subscriber graph).
//   * `on_focus_changed` — fired when the element gains or loses focus.
//                          The `focused` parameter is `true` for focus,
//                          `false` for blur. Elements use this to manage
//                          async tasks tied to focus (e.g. caret blink).
//   * `before_destroy`   — fired once, immediately before the element is
//                          removed from the tree (in `destroy_subtree`).
// ---------------------------------------------------------------------------

pub trait Lifecycle {
    fn on_mounted(&mut self, _cx: &mut SharedViewCx, _boa: &mut Context) {}
    fn on_updated(&mut self, _cx: &mut SharedViewCx, _boa: &mut Context) {}
    fn on_focus_changed(&mut self, _focused: bool, _cx: &mut SharedViewCx, _boa: &mut Context) {}
    fn before_destroy(&mut self, _cx: &mut SharedViewCx, _boa: &mut Context) {}
}
