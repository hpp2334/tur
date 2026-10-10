use std::marker::PhantomData;

use crate::core::edgy::reactive::Mutation;

// ---------------------------------------------------------------------------
// MutationHandle<E> — atom-backed callback handle (Copy).
//
// Stores a `Mutation` typed handle; the closure itself lives in the reactive
// `Store`'s closures map and is resolved at flush time via `invoke_mutation`.
// The `E` parameter is a phantom documentation type (the event payload the
// callback receives as its native `Value` args).
// ---------------------------------------------------------------------------

pub struct MutationHandle<E> {
    mutation: Mutation,
    _marker: PhantomData<fn() -> E>,
}

impl<E> MutationHandle<E> {
    pub fn new(mutation: Mutation) -> Self {
        MutationHandle {
            mutation,
            _marker: PhantomData,
        }
    }

    pub fn mutation(&self) -> Mutation {
        self.mutation
    }
}

impl<E> Clone for MutationHandle<E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E> Copy for MutationHandle<E> {}
