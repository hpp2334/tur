use crate::core::edgy::mutation::MutationHandle;
use crate::core::elements::AnyElement;
use crate::core::focus::{BlurEvent, FocusEvent};

// ---------------------------------------------------------------------------
// Focusable — the contract implemented by element types that can carry
// focus / blur callbacks. Each element owns *how* it stores its callback
// (a field on the spec, or a controller accessor); the focus domain only
// needs to know *which* elements are focusable.
//
// Elements register themselves as focusable at construction time via
// `AnyElement::with_focusable::<T>()`, which stores a fn-pointer that
// downcasts the erased element to `&dyn Focusable`. This keeps `AnyElement`
// free of concrete focusable-type knowledge.
// ---------------------------------------------------------------------------

pub trait Focusable: 'static {
    fn on_focus_mutation(&self) -> Option<MutationHandle<FocusEvent>>;
    fn on_blur_mutation(&self) -> Option<MutationHandle<BlurEvent>>;
}

/// Resolve a type-erased element to its `Focusable` face, if it registered
/// itself as focusable via `AnyElement::with_focusable::<T>()` at
/// construction time.
pub fn as_focusable(elem: &AnyElement) -> Option<&dyn Focusable> {
    elem.as_focusable()
}
