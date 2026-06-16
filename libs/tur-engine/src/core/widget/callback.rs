use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::{js_string, Context, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::reactive::{extract_atom, AtomId, Store};

// ---------------------------------------------------------------------------
// EventArg — convert an event to its JS callback arguments.
//
// Implementations live alongside their event structs in each event's owning
// module (e.g. keyboard events in core/keyboard, scroll events in
// core/scroll, pointer events in elements/pointer_interact).
// ---------------------------------------------------------------------------

pub trait EventArg: 'static {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue>;
}

// ---------------------------------------------------------------------------
// ReturnVal — parse a JS return value into a Rust type.  Defaults to ().
// ---------------------------------------------------------------------------

pub trait ReturnVal: Default + Clone + 'static {
    fn from_js_return(v: &JsValue, ctx: &mut Context) -> Self;
}

impl ReturnVal for () {
    fn from_js_return(_v: &JsValue, _ctx: &mut Context) -> Self {}
}

// ---------------------------------------------------------------------------
// Mutation<E, R> — spec-level atom-backed callback handle (Copy, no JsValues).
// ---------------------------------------------------------------------------

pub struct Mutation<E: EventArg, R: ReturnVal = ()> {
    pub(crate) id: AtomId,
    _marker: PhantomData<fn(E) -> R>,
}

// Manual Clone/Copy — PhantomData<fn(E) -> R> is always Copy regardless of
// E/R, so we don't need E: Clone/Copy bounds.
impl<E: EventArg, R: ReturnVal> Clone for Mutation<E, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E: EventArg, R: ReturnVal> Copy for Mutation<E, R> {}

impl<E: EventArg, R: ReturnVal> Mutation<E, R> {
    pub fn new(id: AtomId) -> Self {
        Mutation {
            id,
            _marker: PhantomData,
        }
    }

    pub fn id(&self) -> AtomId {
        self.id
    }
}

pub fn mutation_from_js<E: EventArg, R: ReturnVal>(v: &JsValue) -> Option<Mutation<E, R>> {
    extract_atom(v).map(Mutation::new)
}

// ---------------------------------------------------------------------------
// Callback<E, R> — controller-level function-backed callback (GC-traced).
// ---------------------------------------------------------------------------

#[derive(Clone)]
#[allow(dead_code)]
pub struct Callback<E: EventArg, R: ReturnVal = ()> {
    pub(crate) func: JsFunction,
    _marker: PhantomData<fn(E) -> R>,
}

impl<E: EventArg, R: ReturnVal> Callback<E, R> {
    pub fn from_function(func: JsFunction) -> Self {
        Callback {
            func,
            _marker: PhantomData,
        }
    }

    pub fn func(&self) -> &JsFunction {
        &self.func
    }
}

// Manual Trace — `empty_trace` matches the controller pattern.
// JsFunctions are kept alive by the controller's GC lifetime, not by tracing.
unsafe impl<E: EventArg, R: ReturnVal> Trace for Callback<E, R> {
    boa_gc::empty_trace!();
}

impl<E: EventArg, R: ReturnVal> Finalize for Callback<E, R> {}

pub fn callback_from_js<E: EventArg, R: ReturnVal>(v: &JsValue) -> Option<Callback<E, R>> {
    v.as_object()
        .and_then(JsFunction::from_object)
        .map(Callback::from_function)
}

/// Get a `Callback<E>` from a JS options object by key (controller-side
/// analogue of `prop_mutation` for specs).
pub fn extract_callback_from_opts<E: EventArg, R: ReturnVal>(
    opts: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<Callback<E, R>> {
    let val = opts.get(js_string!(key), ctx).ok()?;
    callback_from_js(&val)
}

// ---------------------------------------------------------------------------
// make_mutation_callback — resolve a Mutation<E> into a JsFunction at emit
// time.  The returned function prepends the store context object and invokes
// the mutation atom by ID.
// ---------------------------------------------------------------------------

pub fn make_mutation_callback<E: EventArg, R: ReturnVal>(
    store: &Rc<RefCell<Store>>,
    boa: &mut Context,
    mutation: &Mutation<E, R>,
) -> JsFunction {
    let atom_id = mutation.id;
    let store_ctx_obj =
        crate::core::reactive::build_store_context_object(boa, store.clone())
            .ok()
            .map(JsValue::from);
    let store_for_closure = store.clone();
    let callback = unsafe {
        boa_engine::native_function::NativeFunction::from_closure(move |_this, args, ctx| {
            let mut full_args = Vec::with_capacity(args.len() + 1);
            if let Some(ref obj) = store_ctx_obj {
                full_args.push(obj.clone());
            }
            full_args.extend_from_slice(args);
            let _ = store_for_closure.borrow().invoke_mutation(atom_id, &full_args, ctx)?;
            Ok(JsValue::undefined())
        })
    };
    boa_engine::object::FunctionObjectBuilder::new(boa.realm(), callback).build()
}
