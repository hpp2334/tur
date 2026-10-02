//! The playground's rut compile service — the `TurRutPlaygroundPlugin`.
//!
//! A tiny plugin whose `register` pushes a [`RutPkgExt`](tur_engine::core::
//! rut_runtime::RutPkgExt) adding the `pg_*` rows to the `tur` host pkg:
//!
//! - `pg_compile(source) -> u64` — compile a rut module in-realm (a fresh
//!   session against the same `tur` decl surface the engine mounts) and, on
//!   success, register it as a virtual-app source
//!   ([`VirtualState::create_source`]), answering the source id (a plain
//!   `u64` — the rut realm lifts it through `va_source_handle`). Answers
//!   `0` on failure.
//! - `pg_compile_diags() -> str` — the string-diagnostics path: the last
//!   compile's parse report (empty when the last compile succeeded).
//!
//! No source string ever round-trips back through rut — the compile
//! registers the source on the parent's virtual-app state directly.

use std::rc::Rc;

use tur_engine::builtin_plugins::virtual_app::VirtualState;
use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::core::rut_runtime::{RutHandles, RutPkgCx, RutPkgExt};
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
    cx.decl.extend(vec![
        (
            "pg_compile".to_string(),
            vec![rut_core::types::TY_STR],
            rut_core::types::TY_U64,
            false,
        ),
        (
            "pg_compile_diags".to_string(),
            vec![],
            rut_core::types::TY_STR,
            false,
        ),
    ]);
    let Some(handles) = cx.handles else {
        // The compile-time decl probe — bodies install at boot only.
        return;
    };
    // Snapshot the instance's live ext surface (every registered plugin's
    // rows + consts + preludes — the element families, animation, …). The
    // layering law moved the family rows out of core's decl module, so the
    // realm must apply the extensions to see the surface a boot binds.
    let exts = handles.inst.rut_pkg_exts.borrow().clone();
    // The last compile's diagnostics ("" while the last compile succeeded).
    let diags = Rc::new(std::cell::RefCell::new(String::new()));

    // pg_compile(source) -> u64 — 0 on failure (diags via pg_compile_diags).
    let d = diags.clone();
    let h = handles.clone();
    rut_vm::pkg_fn!(cx.pkg, "pg_compile", (&str,) -> u64, move |_vm: &mut rut_vm::interp::Vm, source: &str| {
        match compile_and_register(source, &exts, &h) {
            Ok(id) => {
                *d.borrow_mut() = String::new();
                id
            }
            Err(report) => {
                *d.borrow_mut() = report;
                0
            }
        }
    });

    // pg_compile_diags() -> str — the last compile's parse report.
    rut_vm::pkg_fn!(cx.pkg, "pg_compile_diags", () -> String, move |_vm: &mut rut_vm::interp::Vm| {
        diags.borrow().clone()
    });
}

/// Compile `source` against the engine's standard assembly —
/// `RutRuntime::parse_check` (std core + the `tur` decl pkg extended with
/// every plugin's rows/consts + the extension preludes: the kit et al.) —
/// then register it as a virtual-app source. Answers the source id, or the
/// joined diagnostics / service error.
fn compile_and_register(
    source: &str,
    exts: &[RutPkgExt],
    handles: &Rc<RutHandles>,
) -> Result<u64, String> {
    if let Err(err) = tur_engine::core::rut_runtime::RutRuntime::parse_check(source, exts) {
        return Err(match err {
            tur_engine::core::app::ModuleError::Parse(diags) => diags,
            other => other.to_string(),
        });
    }
    let state = handles
        .inst
        .plugin_state::<VirtualState>()
        .ok_or_else(|| "the virtual-app plugin is not registered on this instance".to_string())?;
    Ok(state.create_source(std::sync::Arc::from(source.to_string())))
}
