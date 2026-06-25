use std::marker::PhantomData;

use crate::core::reactive::Mutation;

use super::EventArg;

// ---------------------------------------------------------------------------
// EdgyMutation<E> — atom-backed callback handle (Copy, no JsValues).
//
// Stores a `Mutation` typed handle; the closure itself lives in the reactive
// `Store`'s closures map and is resolved at flush time via `invoke_mutation`.
// ---------------------------------------------------------------------------

pub struct EdgyMutation<E: EventArg> {
    pub(crate) mutation: Mutation,
    _marker: PhantomData<fn() -> E>,
}

impl<E: EventArg> EdgyMutation<E> {
    pub fn new(mutation: Mutation) -> Self {
        EdgyMutation { mutation, _marker: PhantomData }
    }

    pub fn mutation(&self) -> Mutation {
        self.mutation
    }
}

impl<E: EventArg> Clone for EdgyMutation<E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E: EventArg> Copy for EdgyMutation<E> {}
