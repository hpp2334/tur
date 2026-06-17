use std::marker::PhantomData;

use crate::core::reactive::AtomId;

use super::EventArg;

// ---------------------------------------------------------------------------
// EdgyMutation<E> — atom-backed callback handle (Copy, no JsValues).
//
// Mirrors the former `Mutation<E, R>` with `R` dropped (no ReturnVal). The
// handle stores only an `AtomId`; the closure itself lives in the reactive
// `Store`'s closures map and is resolved at flush time via `invoke_mutation`.
// ---------------------------------------------------------------------------

pub struct EdgyMutation<E: EventArg> {
    pub(crate) id: AtomId,
    _marker: PhantomData<fn() -> E>,
}

impl<E: EventArg> EdgyMutation<E> {
    pub fn new(id: AtomId) -> Self {
        EdgyMutation { id, _marker: PhantomData }
    }

    pub fn id(&self) -> AtomId {
        self.id
    }
}

impl<E: EventArg> Clone for EdgyMutation<E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E: EventArg> Copy for EdgyMutation<E> {}
