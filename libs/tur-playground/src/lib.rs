//! The playground's rut compile service — the `TurRutPlaygroundPlugin`.
//!
//! A tiny plugin whose `register` pushes a [`RutPkgExt`](tur_engine::core::
//! rut_runtime::RutPkgExt) adding the `pg_*` rows to the `tur_host` pkg:
//!
//! - `pg_files_put(path, text) -> nil` — stage one module file into the
//!   per-instance scratch: the key is a mod path (the declaration
//!   grammar's paths — "landing", "net/tls"); the ROOT never appears —
//!   it rides `pg_compile`'s argument.
//! - `pg_compile(root) -> u64` — consume the staged file tree (the root
//!   source argument + every `pg_files_put`-staged mod child; a compile
//!   with no puts is the root-only single-file shape) in-realm (a fresh
//!   session against the same `tur_host` decl surface the engine mounts)
//!   and, on success, register it as a virtual-app source
//!   ([`VirtualState::create`]), answering the source id (a plain
//!   `u64` — the rut realm lifts it through `va_source_handle`). Answers
//!   `0` on failure.
//! - `pg_compile_diags() -> str` — the string-diagnostics path: the last
//!   compile's parse report (empty when the last compile succeeded).
//! - `pg_highlight(src) -> opaque` — classify rut source (rut-semantic:
//!   token + AST name classes) into the editor's colored span run, sealed
//!   as an opaque ([`PgSpans`]).
//! - `pg_apply_highlight(ctrl, spans) -> nil` — write the run into an
//!   editor controller (the `tctrl_*` refresh law: mark the mounted
//!   editable dirty + request a frame). The apply preserves the caret: it
//!   is a re-highlight, not a value write (see the row body's comment).
//!
//! No source string ever round-trips back through rut — the compile
//! registers the source on the parent's virtual-app state directly.

mod highlight;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use rut_driver::HostRow;
use rut_vm::Opaque;

use tur_engine::builtin_plugins::text::{RutTextCtrl, controller::SpanData};
use tur_engine::builtin_plugins::virtual_app::VirtualState;
use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::core::rut_runtime::{RutHandles, RutModuleFiles, RutPkgCx, RutPkgExt};
use tur_engine::error::TurError;

use crate::highlight::{PgSpans, highlight_spans};

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
        HostRow::root(
            "pg_compile",
            vec![rut_core::types::TY_STR],
            rut_core::types::TY_U64,
            false,
        ),
        HostRow::root(
            "pg_compile_diags",
            vec![],
            rut_core::types::TY_STR,
            false,
        ),
        HostRow::root(
            "pg_files_put",
            vec![rut_core::types::TY_STR, rut_core::types::TY_STR],
            rut_core::types::TY_NIL,
            false,
        ),
        HostRow::root(
            "pg_highlight",
            vec![rut_core::types::TY_STR],
            rut_core::types::TY_OPAQUE,
            false,
        ),
        HostRow::root(
            "pg_apply_highlight",
            vec![rut_core::types::TY_OPAQUE, rut_core::types::TY_OPAQUE],
            rut_core::types::TY_NIL,
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
    // The staged-files scratch: `pg_files_put` accumulates the module's
    // mod children here (mod path → text); `pg_compile` consumes the
    // whole map as its `RutModuleFiles.mods`. A compile with no puts is
    // the root-only single-file shape — one uniform lane, no second row.
    let staged = Rc::new(RefCell::new(BTreeMap::<String, String>::new()));

    // pg_files_put(path, text) -> nil — stage one module file (a mod
    // path; the root never appears — it rides pg_compile's argument).
    let staged_put = staged.clone();
    rut_vm::pkg_fn!(cx.pkg, "pg_files_put", (&str, &str) -> (), move |_vm: &mut rut_vm::interp::Vm, path: &str, text: &str| {
        staged_put
            .borrow_mut()
            .insert(path.to_string(), text.to_string());
    });

    // pg_compile(root) -> u64 — CONSUME the staged file tree (the root
    // source argument + every pg_files_put-staged mod child) and compile
    // it; 0 on failure (diags via pg_compile_diags). The take is
    // unconditional: a consumed-on-compile scratch, failed or not.
    let d = diags.clone();
    let staged_compile = staged.clone();
    let h = handles.clone();
    rut_vm::pkg_fn!(cx.pkg, "pg_compile", (&str,) -> u64, move |_vm: &mut rut_vm::interp::Vm, root: &str| {
        let staged = std::mem::take(&mut *staged_compile.borrow_mut());
        let files = RutModuleFiles {
            root: Arc::from(root),
            mods: staged.into_iter().map(|(p, t)| (Arc::from(p), Arc::from(t))).collect(),
        };
        match compile_and_register(files, &exts, &h) {
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

    // pg_highlight(src) -> opaque — tokenize the source into the editor's
    // colored span run (sealed [`PgSpans`]).
    rut_vm::pkg_fn!(cx.pkg, "pg_highlight", (&str,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, src: &str| {
        Ok(Opaque::alloc(vm, PgSpans(highlight_spans(src)))?.handle().clone())
    });

    // pg_apply_highlight(ctrl, spans) -> nil — write the colored run into
    // the editor controller, then the tctrl write law (mark the mounted
    // editable dirty + request a frame). The apply PRESERVES the caret
    // (`set_spans_preserve_cursor`, the re-tokenization arm): `set_spans`
    // would yank it to EOF, and with auto-run on, a Run re-highlight fires
    // between keystrokes — phase-D decision: highlight at case load +
    // after a successful Run, never per keystroke, caret kept. The
    // no-op-rehighlight-must-not-push-undo law holds in the controller
    // (same-text applies push nothing, identical spans bump nothing).
    let h = handles.clone();
    rut_vm::pkg_fn!(cx.pkg, "pg_apply_highlight", (Opaque<RutTextCtrl>, Opaque<PgSpans>) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, s: Opaque<PgSpans>| {
        let spans: Vec<SpanData> = s.with(|s| s.0.clone())?;
        let handles = h.clone();
        c.with_mut(vm, |_vm, c| {
            let mut ctrl = c.0.borrow_mut();
            ctrl.set_spans_preserve_cursor(spans);
            if let Some(id) = ctrl.mounted_view() {
                handles.element_tree.mark_dirty(id);
            }
        })?;
        (handles.request_frame)();
        Ok(())
    });
}

/// Compile the module FILE TREE against the engine's standard assembly —
/// `RutRuntime::parse_check_files` (std core + the `tur_host` decl pkg extended
/// with every plugin's rows/consts + the extension preludes: the kit et al.) —
/// then register it as a virtual-app source. Answers the source id, or the
/// joined diagnostics / service error. A root-only tree (no staged puts)
/// is the historical single-string shape.
fn compile_and_register(
    files: RutModuleFiles,
    exts: &[RutPkgExt],
    handles: &Rc<RutHandles>,
) -> Result<u64, String> {
    if let Err(err) = tur_engine::core::rut_runtime::RutRuntime::parse_check_files(&files, exts) {
        return Err(match err {
            tur_engine::core::app::ModuleError::Parse(diags) => diags,
            other => other.to_string(),
        });
    }
    let state = handles
        .inst
        .plugin_state::<VirtualState>()
        .ok_or_else(|| "the virtual-app plugin is not registered on this instance".to_string())?;
    Ok(state.create(files))
}
