//! The playground's rut compile service — the `TurPlaygroundPlugin` (swc
//! TS compiler) replacement for the rut rail.
//!
//! A tiny plugin whose `register` pushes a [`RutPkgExt`](tur_engine::core::
//! rut_runtime::RutPkgExt) adding the `pg_*` rows to the `tur` host pkg:
//!
//! - `pg_compile(source) -> str` — compile a rut module in-realm (a fresh
//!   session against the same `tur` decl surface the engine mounts) and
//!   answer `"ok"` or the joined parse diagnostics.

use std::rc::Rc;

use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::core::rut_runtime::{RutPkgCx, RutPkgExt};
use tur_engine::error::TurError;

#[derive(Default)]
pub struct TurRutPlaygroundPlugin;

impl Plugin for TurRutPlaygroundPlugin {
    fn register(&self, ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
        let ext: RutPkgExt = Rc::new(|cx: &mut RutPkgCx<'_>| install(cx));
        ctx.js_ctx().rut_pkg_exts.borrow_mut().push(ext);
        Ok(())
    }
}

fn install(cx: &mut RutPkgCx<'_>) {
    cx.decl.extend(vec![(
        "pg_compile".to_string(),
        vec![rut_core::types::TY_STR],
        rut_core::types::TY_STR,
        false,
    )]);
    let Some(_handles) = cx.handles else {
        // The compile-time decl probe — bodies install at boot only.
        return;
    };
    rut_vm::pkg_fn!(cx.pkg, "pg_compile", (&str,) -> String, |_vm: &mut rut_vm::interp::Vm, source: &str| {
        compile_in_realm(source)
    });
}

/// Compile `source` as a rut module against the engine's `tur` surface —
/// the same assembly `RutRuntime::compile` uses (std core + the async
/// weave + the `tur` decl pkg). Answers `"ok"` or the joined diags.
fn compile_in_realm(source: &str) -> String {
    let mut session = rut_driver::Session::new();
    rut_driver::mount_std_core(&mut session);
    // The async weave is native-only for now (see
    // `core::rut_runtime::RutRuntime::compile` — the weave's mount reads
    // the toolchain tree from disk).
    #[cfg(not(target_arch = "wasm32"))]
    rut_driver::mount_std_async(&mut session);
    if let Err(e) = session.register_module("tur", tur_engine::core::rut_runtime::tur_decl_module()) {
        return format!("mount tur pkg: {e}");
    }
    let out = rut_driver::compile_module_in(&mut session, source, rut_parser::Mode::Impl, "case");
    if out.diags.is_empty() {
        match out.binary {
            Some(_) => "ok".to_string(),
            None => "compile emitted no binary".to_string(),
        }
    } else {
        out.diags
            .iter()
            .map(|d| d.msg.clone())
            .collect::<Vec<_>>()
            .join("; ")
    }
}
