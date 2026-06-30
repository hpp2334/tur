use crate::core::edgy_event::EdgyMutation;
use crate::core::elements::AnyElement;
use crate::core::focus::{BlurEvent, FocusEvent};

// ---------------------------------------------------------------------------
// Focusable — the contract implemented by element types that can carry
// focus / blur callbacks. Each element owns *how* it stores its callback
// (a field on the spec, or a controller accessor); the focus domain only
// needs to know the *set* of focusable element types (see `as_focusable`).
//
// This keeps `AnyElement` domain-neutral: focus-specific queries live behind
// this trait in the focus domain, not as methods on the erased element.
// ---------------------------------------------------------------------------

pub trait Focusable: 'static {
    fn on_focus_mutation(&self) -> Option<EdgyMutation<FocusEvent>>;
    fn on_blur_mutation(&self) -> Option<EdgyMutation<BlurEvent>>;
}

/// Resolve a type-erased element to its `Focusable` face, if it is one of the
/// element types that can carry focus callbacks. Adding a new focusable
/// element type = implement `Focusable` on it and add it to this list.
///
/// `AnyElement::cast` only downcasts to concrete types (not trait objects), so
/// the set of focusable types is enumerated here, in the domain that owns
/// focusability — rather than as erased fn-pointers on `AnyElement`.
pub(crate) fn as_focusable(elem: &AnyElement) -> Option<&dyn Focusable> {
    use crate::elements::{EditableTextElement, FocusableElement};
    elem.cast::<FocusableElement>()
        .map(|f| f as &dyn Focusable)
        .or_else(|| elem.cast::<EditableTextElement>().map(|e| e as &dyn Focusable))
}
