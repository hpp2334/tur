use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::property::PropertyDescriptor;
use boa_engine::{js_string, Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

mod store;

pub use store::Store;

/// Unique identifier for a reactive atom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtomId(pub u32);

impl AtomId {
    #[inline]
    pub fn new(id: u32) -> Self {
        AtomId(id)
    }

    #[inline]
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Kind of atom — drives how the store treats it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomKind {
    Source,
    Derived,
    Mutation,
}

/// Opaque handle returned to JS for an atom.
#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct AtomHandle {
    pub id: AtomId,
}

impl AtomHandle {
    pub fn new(id: AtomId) -> Self {
        AtomHandle { id }
    }
}

/// Extract an AtomId from a JS value wrapping an [`AtomHandle`].
pub fn extract_atom(value: &JsValue) -> Option<AtomId> {
    value.as_object().and_then(|obj| {
        obj.downcast_ref::<AtomHandle>().map(|h| h.id)
    })
}

/// Extract an atom id or raise a TypeError.
pub fn require_atom(args: &[JsValue], idx: usize) -> JsResult<AtomId> {
    extract_atom(args.get_or_undefined(idx)).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected an atom handle"))
    })
}

/// Build the per-store `{ get, set }` JS context object that closures receive
/// as their first argument.
pub fn build_store_context_object(
    context: &mut Context,
    store: Rc<RefCell<Store>>,
) -> JsResult<JsObject> {
    let proto = context.intrinsics().constructors().object().prototype();
    let obj = JsObject::from_proto_and_data(proto, ());

    let store_for_get = store.clone();
    let get_fn = unsafe {
        boa_engine::native_function::NativeFunction::from_closure(move |_this, args, ctx| {
            let id = require_atom(args, 0)?;
            Ok(store_for_get.borrow().get_tracked(id, ctx))
        })
    };
    let get_obj = boa_engine::object::FunctionObjectBuilder::new(context.realm(), get_fn)
        .name(js_string!("get"))
        .length(1)
        .build();
    let get_desc = PropertyDescriptor::builder()
        .value(get_obj)
        .writable(true)
        .enumerable(false)
        .configurable(true)
        .build();
    obj.insert_property(js_string!("get"), get_desc);

    let store_for_set = store.clone();
    let set_fn = unsafe {
        boa_engine::native_function::NativeFunction::from_closure(move |_this, args, ctx| {
            let store = store_for_set.borrow();
            let id = require_atom(args, 0)?;
            match store.kind_of(id) {
                Some(AtomKind::Mutation) => {
                    // Prepend the store ctx object so mutation closures
                    // invoked via the per-store `set(mutation, ...args)`
                    // helper receive `(ctx, ...args)` — matching the
                    // event-flush dispatch contract.
                    let ctx_obj = build_store_context_object(ctx, store_for_set.clone())?;
                    let mut invoke_args: Vec<JsValue> = Vec::with_capacity(args.len() + 1);
                    invoke_args.push(ctx_obj.into());
                    if let Some(extra) = args.get(1..) {
                        invoke_args.extend_from_slice(extra);
                    }
                    store.invoke_mutation(id, &invoke_args, ctx)
                }
                _ => {
                    let value = args.get_or_undefined(1).clone();
                    store.set_source(id, value);
                    Ok(JsValue::undefined())
                }
            }
        })
    };
    let set_obj = boa_engine::object::FunctionObjectBuilder::new(context.realm(), set_fn)
        .name(js_string!("set"))
        .length(2)
        .build();
    let set_desc = PropertyDescriptor::builder()
        .value(set_obj)
        .writable(true)
        .enumerable(false)
        .configurable(true)
        .build();
    obj.insert_property(js_string!("set"), set_desc);

    Ok(obj)
}

/// Convenience alias used by some widget helpers — `Attribute::all()` etc.
#[allow(dead_code)]
fn _use_attribute() {}
