//! Native ES module loader for the tur bridge.
//!
//! Replaces the old `globalThis.__tur` global object. Engine-owned bridge
//! functions are exposed as synthetic modules (`tur:core` for the
//! reactive substrate, `tur:std` for the widget library) that user
//! code imports directly:
//!
//! ```js
//! import { source, render } from "tur:core";
//! import { Container, Color } from "tur:std";
//! ```
//!
//! The loader keeps a registry of bare `tur:*` specifiers → pre-built
//! [`Module`]s. Plugins register additional capability modules
//! (`tur:net`, `tur:clipboard`, …) through
//! [`PluginContext::register_module`] / [`PluginContext::register_host_module`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::Context;
use boa_engine::JsNativeError;
use boa_engine::JsObject;
use boa_engine::JsResult;
use boa_engine::JsValue;
use boa_engine::Module;
use boa_engine::NativeFunction;
use boa_engine::js_string;
use boa_engine::module::ModuleLoader;
use boa_engine::module::ModuleRequest;
use boa_engine::module::Referrer;
use boa_engine::module::SyntheticModuleInitializer;
use boa_engine::native_function::NativeFunctionPointer;
use boa_engine::object::FunctionObjectBuilder;

/// A module loader that resolves registered bare specifiers (e.g. `tur:std`)
/// to pre-built [`Module`]s. Unknown specifiers raise a `TypeError`.
///
/// Held behind an `Rc` and shared between [`crate::TurApp`] (which wires it
/// into the boa `Context`) and the bridge (which registers modules into it).
#[derive(Default)]
pub struct TurModuleLoader {
    modules: RefCell<HashMap<String, Module>>,
}

impl TurModuleLoader {
    /// Create an empty loader, ready to be handed to
    /// `Context::builder().module_loader(...)`.
    #[must_use]
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// Register (or replace) a module under a bare specifier.
    pub fn register(&self, specifier: &str, module: Module) {
        self.modules
            .borrow_mut()
            .insert(specifier.to_string(), module);
    }

    /// Resolve a specifier to a registered module, if any.
    pub fn get(&self, specifier: &str) -> Option<Module> {
        self.modules.borrow().get(specifier).cloned()
    }
}

impl ModuleLoader for TurModuleLoader {
    async fn load_imported_module(
        self: Rc<Self>,
        _referrer: Referrer,
        request: ModuleRequest,
        _context: &RefCell<&mut Context>,
    ) -> JsResult<Module> {
        let spec = request.specifier().to_std_string_escaped();
        if let Some(m) = self.modules.borrow().get(&spec) {
            Ok(m.clone())
        } else {
            Err(JsNativeError::typ()
                .with_message(format!("unknown module specifier: {spec}"))
                .into())
        }
    }
}

/// Build a synthetic module whose exports are native bridge functions.
///
/// Each `(name, length, ptr)` in `fns` becomes a *ctx-bound* export: a thin
/// wrapper that prepends `ctx_value` to the JS arguments and forwards to the
/// fn pointer — so the existing ctx-first native implementations
/// (`tur_source(ctx, value)`, `tur_container(ctx, props)`, …) are reused
/// verbatim, while JS callers get a ctx-free surface
/// (`source(value)`, `Container(props)`).
///
/// Each `(name, length, nf)` in `closures` becomes a *free-form* export: the
/// `NativeFunction` is registered as-is, with no ctx prepending. Used for
/// bridge fns that need to capture state that can't live on `TurJsContext`
/// (e.g. clipboard/http impls from outside tur-engine).
///
/// Each `(name, val)` in `consts` becomes a constant export.
pub fn build_native_module(
    context: &mut Context,
    ctx_value: JsValue,
    fns: &[(&str, usize, NativeFunctionPointer)],
    closures: &[(&str, usize, NativeFunction)],
    consts: &[(&str, JsValue)],
) -> Module {
    // Collect every export as a (name, value) pair: bound native fns, closure
    // fns, then constant values (enum objects, etc.). A single flat list
    // keeps the synthetic-module initializer trivial.
    let mut exports: Vec<(boa_engine::JsString, JsValue)> =
        Vec::with_capacity(fns.len() + closures.len() + consts.len());
    for (name, length, ptr) in fns {
        let f = bound_native(context, ctx_value.clone(), *ptr, *length, name);
        exports.push((js_string!(*name), f.into()));
    }
    for (name, length, nf) in closures {
        let f = FunctionObjectBuilder::new(context.realm(), nf.clone())
            .length(*length)
            .name(js_string!(*name))
            .build();
        exports.push((js_string!(*name), f.into()));
    }
    for (name, val) in consts {
        exports.push((js_string!(*name), val.clone()));
    }

    let export_names: Vec<boa_engine::JsString> = exports.iter().map(|(n, _)| n.clone()).collect();

    let init = SyntheticModuleInitializer::from_copy_closure_with_captures(
        |module, exports, _ctx| {
            for (name, val) in exports {
                module.set_export(name, val.clone())?;
            }
            Ok(())
        },
        exports,
    );

    Module::synthetic(&export_names, init, None, None, context)
}

/// Build a synthetic module whose exports are arbitrary native functions
/// (no bridge-ctx binding). Used by plugins to expose host services as
/// modules — e.g. `tur:net` (HTTP `request`), `tur-ext/demo-helper`
/// (swc compiler + file IO).
pub fn build_fn_module(context: &mut Context, exports: &[(&str, NativeFunction, usize)]) -> Module {
    let export_names: Vec<boa_engine::JsString> = exports
        .iter()
        .map(|(name, _, _)| js_string!(*name))
        .collect();

    let fns: Vec<JsObject> = exports
        .iter()
        .map(|(name, f, len)| {
            FunctionObjectBuilder::new(context.realm(), f.clone())
                .length(*len)
                .name(js_string!(*name))
                .build()
                .into()
        })
        .collect();

    let init = SyntheticModuleInitializer::from_copy_closure_with_captures(
        |module, (names, fns), _ctx| {
            for (name, f) in names.iter().zip(fns.iter()) {
                module.set_export(name, f.clone().into())?;
            }
            Ok(())
        },
        (export_names.clone(), fns),
    );

    Module::synthetic(&export_names, init, None, None, context)
}

/// Build a single bound native function object: `f(args...)` ≡
/// `ptr(ctx_value, args...)`.
pub fn bound_native(
    context: &mut Context,
    ctx_value: JsValue,
    ptr: NativeFunctionPointer,
    length: usize,
    name: &str,
) -> JsObject {
    let f = NativeFunction::from_copy_closure_with_captures(
        move |this, args, ctx_v, context| {
            // Prepend the captured bridge ctx so the underlying ctx-first
            // native fn is called with its expected first argument.
            let mut full: Vec<JsValue> = Vec::with_capacity(args.len() + 1);
            full.push(ctx_v.clone());
            full.extend_from_slice(args);
            ptr(this, &full, context)
        },
        ctx_value,
    );
    FunctionObjectBuilder::new(context.realm(), f)
        .length(length)
        .name(js_string!(name))
        .build()
        .into()
}
