use std::rc::Rc;

use crate::core::element::NodeId;

pub mod build_cx;
pub mod context;
pub mod val;

pub use build_cx::{ViewCx, read_val, read_val_opt};
pub use context::SharedViewCx;
pub use val::Val;

// ---------------------------------------------------------------------------
// View — the user's declaration of a view.
//
// Pure Rust data: reactive props are `Val<T>` and children are
// `Vec<Rc<dyn View>>`. `build()` instantiates the view into the node tree.
// Views are immutable after creation.
//
// `build` takes `&mut dyn ViewCx` so the trait stays object-safe (`dyn View`
// is used by builders) while still accepting either a `SharedViewCx`
// (normal builds) or a layout-backed `ViewCx` impl (build-during-layout).
// ---------------------------------------------------------------------------

pub trait View: 'static {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId;
}

// ---------------------------------------------------------------------------
// ViewFactory — produces a View on demand.
//
// Used for branches whose concrete subtree is only determined at runtime
// (Condition/Switch branches). The factory is retained and `create()` is
// invoked when the branch is selected, and re-invoked on a branch swap.
// Rust factories (the rut rail's pre-built branches) clone an `Rc`.
// ---------------------------------------------------------------------------

pub trait ViewFactory: 'static {
    fn create(&self) -> Option<Rc<dyn View>>;
}

// ---------------------------------------------------------------------------
// Lifecycle — optional element lifecycle hooks. All default to no-op
// so every element type satisfies the bound without boilerplate.
//
//   * `on_mounted`       — fired once, right after the element is inserted
//                          into the tree (in `SharedViewCx::insert_node`).
//   * `on_focus_changed` — fired when the element gains or loses focus.
//                          The `focused` parameter is `true` for focus,
//                          `false` for blur. Elements use this to manage
//                          async tasks tied to focus (e.g. caret blink).
//   * `before_destroy`   — fired once, immediately before the element is
//                          removed from the tree (in `destroy_subtree`).
// ---------------------------------------------------------------------------

pub trait Lifecycle {
    fn on_mounted(&mut self, _cx: &mut SharedViewCx) {}
    fn on_focus_changed(&mut self, _focused: bool, _cx: &mut SharedViewCx) {}
    fn before_destroy(&mut self, _cx: &mut SharedViewCx) {}
}
