//! [`JsProps`] — the single accessor every element uses to read its props off a
//! JS object. Replaces the ~15× duplicated `prop_val` / `prop_query_key` /
//! `prop_child` / `prop_children` / `prop_mutation` free functions that used to
//! live in each `elements/<name>/element.rs`.
//!
//! A `JsProps` borrows a [`JsObject`] and a [`Context`] once; each method reads
//! one keyed property and decodes it via the unified [`crate::core::js_value`]
//! traits. Reactive-aware decoding (for [`Val<T>`]) tries an atom handle first,
//! then a static value.

use std::rc::Rc;

use boa_engine::object::builtins::{JsArray, JsFunction};
use boa_engine::object::JsObject;
use boa_engine::{js_string, Context, JsResult, JsValue};

use crate::core::mutation::{mutation_from_js, MutationHandle, IntoJsArgs};
use crate::core::js_value::{type_error, FromJs};
use crate::core::reactive::AnyReadable;
use crate::core::view::{extract_view, val_from_js, JsViewFactory, Val, View, ViewFactory};

/// Borrowed view over a JS props object + the context needed to read it.
///
/// Construct with [`JsProps::new`] inside an element's `from_js`, then call the
/// typed accessors. Each accessor reads one property by key; absent/unreadable
/// props yield `None` (or, for [`JsProps::get`], a typed error).
pub struct JsProps<'a> {
    obj: &'a JsObject,
    ctx: &'a mut Context,
}

impl<'a> JsProps<'a> {
    pub fn new(obj: &'a JsObject, ctx: &'a mut Context) -> Self {
        JsProps { obj, ctx }
    }

    /// Read a single (required) property and decode it. Absent or wrongly-typed
    /// props propagate a `JsError` (use `?` from a `from_js` that returns
    /// `JsResult<Self>`).
    pub fn get<T: FromJs>(&mut self, key: &str) -> JsResult<T> {
        let v = self.obj.get(js_string!(key), self.ctx)?;
        T::from_js(&v)
    }

    /// Read an optional property. Returns `None` if the key is absent, null, or
    /// undefined; decodes otherwise.
    pub fn opt<T: FromJs>(&mut self, key: &str) -> Option<T> {
        let v = self.obj.get(js_string!(key), self.ctx).ok()?;
        if v.is_null() || v.is_undefined() {
            return None;
        }
        T::from_js(&v).ok()
    }

    /// Read a `Val<T>` prop (reactive-or-static). Returns `None` if the key is
    /// absent/null/undefined or the value can't be decoded.
    pub fn val<T: FromJs + Clone + 'static>(&mut self, key: &str) -> Option<Val<T>> {
        let v = self.obj.get(js_string!(key), self.ctx).ok()?;
        val_from_js(&v)
    }

    /// Read a `Vec<String>` prop (e.g. `queryKey`). Returns `None` if absent or
    /// empty. Non-string entries are silently skipped (matches prior behavior).
    pub fn query_key(&mut self, key: &str) -> Option<Vec<String>> {
        let v = self.obj.get(js_string!(key), self.ctx).ok()?;
        let arr = JsArray::from_object(v.as_object()?.clone()).ok()?;
        let len = arr.length(self.ctx).ok()? as usize;
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            if let Ok(val) = arr.at(i as i64, self.ctx) {
                if let Some(s) = val.as_string() {
                    out.push(s.to_std_string_escaped());
                }
            }
        }
        (!out.is_empty()).then_some(out)
    }

    /// Read a single child spec (`ViewHandle` opaque). `None` if absent.
    pub fn child(&mut self, key: &str) -> Option<Rc<dyn View>> {
        let v = self.obj.get(js_string!(key), self.ctx).ok()?;
        extract_view(&v)
    }

    /// Read an array of child specs. Empty vec if absent or not an array;
    /// non-ViewHandle entries are silently skipped.
    pub fn children(&mut self, key: &str) -> Vec<Rc<dyn View>> {
        let Ok(v) = self.obj.get(js_string!(key), self.ctx) else {
            return Vec::new();
        };
        let Some(obj) = v.as_object() else {
            return Vec::new();
        };
        let Ok(arr) = JsArray::from_object(obj.clone()) else {
            return Vec::new();
        };
        let len = arr.length(self.ctx).unwrap_or(0);
        let mut out = Vec::with_capacity(len as usize);
        for i in 0..len {
            if let Ok(item) = arr.at(i as i64, self.ctx) {
                if let Some(spec) = extract_view(&item) {
                    out.push(spec);
                }
            }
        }
        out
    }

    /// Read an atom-backed callback handle (`MutationHandle<E>`) from an atom
    /// handle prop. `None` if absent or not a mutation handle.
    pub fn mutation<E: IntoJsArgs>(&mut self, key: &str) -> Option<MutationHandle<E>> {
        let v = self.obj.get(js_string!(key), self.ctx).ok()?;
        mutation_from_js(&v)
    }

    /// Read an array of atom handles (`AnyReadable`). Empty vec if absent;
    /// non-handle entries are silently skipped.
    pub fn readables(&mut self, key: &str) -> Vec<AnyReadable> {
        let Ok(v) = self.obj.get(js_string!(key), self.ctx) else {
            return Vec::new();
        };
        let Some(obj) = v.as_object() else {
            return Vec::new();
        };
        let Ok(arr) = JsArray::from_object(obj.clone()) else {
            return Vec::new();
        };
        let len = arr.length(self.ctx).unwrap_or(0);
        let mut out = Vec::with_capacity(len as usize);
        for i in 0..len {
            if let Ok(item) = arr.at(i as i64, self.ctx) {
                if let Ok(readable) = AnyReadable::from_js(&item) {
                    out.push(readable);
                }
            }
        }
        out
    }

    /// Read a `[x, y]` pair (e.g. `shadowOffset`). `None` if absent or not a
    /// 2-element numeric array.
    pub fn offset(&mut self, key: &str) -> Option<(f64, f64)> {
        let v = self.obj.get(js_string!(key), self.ctx).ok()?;
        let arr = JsArray::from_object(v.as_object()?.clone()).ok()?;
        let x = arr.at(0, self.ctx).ok()?.as_number()?;
        let y = arr.at(1, self.ctx).ok()?.as_number()?;
        Some((x, y))
    }

    /// Read a branch factory from a JS thunk `() => Element`. `None` if
    /// absent/null/undefined or not callable.
    pub fn factory(&mut self, key: &str) -> Option<Rc<dyn ViewFactory>> {
        let v = self.obj.get(js_string!(key), self.ctx).ok()?;
        if v.is_undefined() || v.is_null() {
            return None;
        }
        let f = v.as_object().and_then(JsFunction::from_object)?;
        Some(Rc::new(JsViewFactory(f)) as Rc<dyn ViewFactory>)
    }

    /// Read a required JS function prop (e.g. an Each/LazyList `build` callback).
    /// `None` if absent or not callable.
    pub fn function(&mut self, key: &str) -> Option<JsFunction> {
        let v = self.obj.get(js_string!(key), self.ctx).ok()?;
        v.as_object().and_then(JsFunction::from_object)
    }

    /// Read a single reactive atom handle (`AnyReadable`). `None` if absent or
    /// not an atom handle.
    pub fn readable(&mut self, key: &str) -> Option<AnyReadable> {
        let v = self.obj.get(js_string!(key), self.ctx).ok()?;
        AnyReadable::from_js(&v).ok()
    }

    /// Read an object prop and downcast its `JsData` payload to `T`. Returns
    /// the `JsObject` only if it carries `T`; `None` otherwise. For controller
    /// handles (`TextEditingController`, `ScrollController`, …).
    pub fn opaque<T: boa_engine::object::NativeObject>(&mut self, key: &str) -> Option<JsObject> {
        let v = self.obj.get(js_string!(key), self.ctx).ok()?;
        let obj = v.as_object()?;
        if obj.downcast_ref::<T>().is_some() {
            Some(obj.clone())
        } else {
            None
        }
    }

    /// Read a raw (required) `JsValue` prop — for opaque controller handles that
    /// don't impl [`FromJs`] but are downcast at the call site.
    pub fn raw(&mut self, key: &str) -> JsResult<JsValue> {
        self.obj.get(js_string!(key), self.ctx)
    }

    /// Read a raw optional `JsValue` prop.
    pub fn raw_opt(&mut self, key: &str) -> Option<JsValue> {
        self.obj.get(js_string!(key), self.ctx).ok()
    }

    /// Borrow the underlying context (for element-specific decoding that
    /// doesn't fit a typed accessor).
    pub fn ctx(&mut self) -> &mut Context {
        self.ctx
    }

    /// Constructor helper for callers that want to surface a uniform
    /// "expected <key>" error without repeating the key string.
    pub fn err(expected: &str) -> boa_engine::JsError {
        type_error(expected)
    }
}
