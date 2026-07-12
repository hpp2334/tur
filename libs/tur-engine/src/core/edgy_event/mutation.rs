use std::marker::PhantomData;

use crate::core::reactive::Mutation;

use super::IntoJsArgs;

// ---------------------------------------------------------------------------
// EdgyMutation<E> — atom-backed callback handle (Copy, no JsValues).
//
// Stores a `Mutation` typed handle; the closure itself lives in the reactive
// `Store`'s closures map and is resolved at flush time via `invoke_mutation`.
// ---------------------------------------------------------------------------

pub struct EdgyMutation<E: IntoJsArgs> {
    mutation: Mutation,
    _marker: PhantomData<fn() -> E>,
}

impl<E: IntoJsArgs> EdgyMutation<E> {
    pub fn new(mutation: Mutation) -> Self {
        EdgyMutation { mutation, _marker: PhantomData }
    }

    pub fn mutation(&self) -> Mutation {
        self.mutation
    }
}

impl<E: IntoJsArgs> Clone for EdgyMutation<E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E: IntoJsArgs> Copy for EdgyMutation<E> {}
