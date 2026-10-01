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
    let Some(handles) = cx.handles else {
        // The compile-time decl probe — bodies install at boot only.
        return;
    };
    // Snapshot the instance's live ext surface (every registered plugin's
    // rows + consts + preludes — the element families, animation, …). The
    // layering law moved the family rows out of core's decl module, so the
    // realm must apply the extensions to see the surface a boot binds.
    let exts = handles.inst.rut_pkg_exts.borrow().clone();
    rut_vm::pkg_fn!(cx.pkg, "pg_compile", (&str,) -> String, move |_vm: &mut rut_vm::interp::Vm, source: &str| {
        compile_in_realm(source, &exts)
    });
}

/// Compile `source` against the engine's standard assembly —
/// `RutRuntime::parse_check` (std core + the `tur` decl pkg extended with
/// every plugin's rows/consts + the extension preludes: the kit et al.).
/// Answers `"ok"` or the joined diags.
fn compile_in_realm(source: &str, exts: &[RutPkgExt]) -> String {
    match tur_engine::core::rut_runtime::RutRuntime::parse_check(source, exts) {
        Ok(()) => "ok".to_string(),
        Err(tur_engine::core::app::ModuleError::Parse(diags)) => diags,
        Err(other) => other.to_string(),
    }
}
