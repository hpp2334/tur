//! C6 — async capabilities over the rut rail: the clipboard rows (the
//! clipboard capability is inlined in the engine) + the bytes helpers.
//!
//! Every async engine API returns `Task<T> = { promise, cancel() }` on the
//! JS side; the rut twin is `pkg_async_fn!` + [`rut_vm::Completer`] — the
//! start closure spawns the capability future on the worker
//! (`js_ctx.spawn_local`) and completes the Completer; the pump's
//! `run_ready` drives the awaiting rut task. Cancel rows land with the
//! capability crates' task handles (net streams wire-abort).
//!
//! The net / filepicker rows live in their capability crates (they own the
//! backend types) and ride the same pkg-extension seam tur-animation uses.

use std::rc::Rc;

use rut_core::types::{TY_BYTES, TY_NIL, TY_STR};

use super::RutHandles;

/// Declare the C6 rows on the `tur_host` decl module. The async rows ride the
/// driver's family expansion (`is_async = true`).
pub fn decl_rows() -> Vec<rut_driver::HostRow> {
    vec![
        ("clipboard_read", vec![], TY_STR, true),
        // The clipboard kit's twin spelling (the `net_request_raw` pattern —
        // the kit's own `clipboard_read` wrapper shadows the row name, so
        // its body reaches the row through the `_raw` alias).
        ("clipboard_read_raw", vec![], TY_STR, true),
        ("clipboard_write", vec![TY_STR], TY_NIL, true),
        ("clipboard_write_raw", vec![TY_STR], TY_NIL, true),
        ("decode_utf8", vec![TY_BYTES], TY_STR, false),
        ("encode_utf8", vec![TY_STR], TY_BYTES, false),
    ]
    .into_iter()
    .map(|(n, p, r, a)| crate::core::rut_runtime::host_row_at("clipboard", n, p, r, a))
    .collect()
}

/// The `clipboard_read` / `clipboard_read_raw` bodies — one law, two
/// spellings (the kit's authored `clipboard_read` wrapper shadows the row
/// name, so it reaches the row through the `_raw` alias).
fn clipboard_read_body(h: Rc<RutHandles>) -> rut_vm::Completer<String> {
    use crate::Clipboard;
    let done = rut_vm::Completer::new();
    let Some(clipboard) = h.inst.capability().of::<Clipboard>() else {
        done.complete(String::new());
        return done;
    };
    let clipboard = clipboard.backend().clone();
    let w = done.clone();
    h.inst.spawn_local(move |_aw| async move {
        w.complete(clipboard.read_text().await);
    });
    done
}

/// The `clipboard_write` / `clipboard_write_raw` bodies (a missing
/// capability completes immediately — the no-op degradation).
fn clipboard_write_body(h: Rc<RutHandles>, text: &str) -> rut_vm::Completer<()> {
    use crate::Clipboard;
    let done = rut_vm::Completer::<()>::new();
    let Some(clipboard) = h.inst.capability().of::<Clipboard>() else {
        done.complete(());
        return done;
    };
    let clipboard = clipboard.backend().clone();
    let text = text.to_string();
    let w = done.clone();
    h.inst.spawn_local(move |_aw| async move {
        clipboard.write_text(text).await;
        w.complete(());
    });
    done
}

/// Install the C6 bodies. (The needless_question_mark allow covers the
/// upstream `pkg_async_fn!` row expansion's cosmetic — the spike's own
/// precedent.)
#[allow(clippy::needless_question_mark)]
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // clipboard_read() -> str — resolves with the platform text (empty when
    // denied/unavailable — the same degradation the JS bridge documents).
    let h = handles.clone();
    rut_vm::pkg_async_fn!(pkg, "clipboard::clipboard_read", () -> String, move || {
        clipboard_read_body(h.clone())
    });

    // The clipboard kit's twin spelling (the `net_request_raw` pattern).
    let h = handles.clone();
    rut_vm::pkg_async_fn!(pkg, "clipboard::clipboard_read_raw", () -> String, move || {
        clipboard_read_body(h.clone())
    });

    // clipboard_write(s: str) -> () — resolves when the write lands.
    let h = handles.clone();
    rut_vm::pkg_async_fn!(pkg, "clipboard::clipboard_write", (&str,) -> (), move |text: &str| {
        clipboard_write_body(h.clone(), text)
    });

    // The clipboard kit's twin spelling (the `net_request_raw` pattern).
    let h = handles.clone();
    rut_vm::pkg_async_fn!(pkg, "clipboard::clipboard_write_raw", (&str,) -> (), move |text: &str| {
        clipboard_write_body(h.clone(), text)
    });

    // ---- bytes helpers (the response-body decode path) ------------------
    rut_vm::pkg_fn!(pkg, "clipboard::decode_utf8", (&[u8],) -> String, |_vm: &mut rut_vm::interp::Vm, bytes: &[u8]| {
        Ok(String::from_utf8_lossy(bytes).into_owned())
    });
    rut_vm::pkg_fn!(pkg, "clipboard::encode_utf8", (&str,) -> Vec<u8>, |_vm: &mut rut_vm::interp::Vm, text: &str| {
        Ok(text.as_bytes().to_vec())
    });
}
