//! C6 — the `tur:net` rut rows (via the pkg-extension seam): `net_request`
//! (async, the body as bytes), `net_stream` (chunk callbacks + a task
//! opaque whose `task_cancel` row wire-aborts the download), and
//! `net_status` (the stashed request status).
//!
//! The JS rail's `Task<T> = { promise, cancel() }` maps to an opaque task
//! + a cancel row; awaits ride `pkg_async_fn!` + [`rut_vm::Completer`].

use std::cell::RefCell;
use std::rc::Rc;

use futures::StreamExt;
use rut_core::types::{TY_BYTES, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};
use rut_vm::Opaque;
use tur_engine::core::rut_runtime::Intent;
use tur_engine::core::scheduler::TaskHandle;

use crate::{Http, HttpOutcome, RequestOpts};

/// The net task opaque: the spawned drive (cancel = drop/abort) + the
/// stashed outcome for the status row.
pub struct RutNetTask {
    handle: TaskHandle,
    pub status: Rc<RefCell<u16>>,
    pub error: Rc<RefCell<String>>,
}

/// Declare the rows + install the bodies (the pkg-extension payload).
/// (The allow covers the upstream `pkg_async_fn!` row expansion's
/// cosmetic — the spike's precedent.)
#[allow(clippy::needless_question_mark)]
pub fn install(cx: &mut tur_engine::core::rut_runtime::RutPkgCx<'_>) {
    cx.decl.extend(
        vec![
            ("net_request", vec![TY_STR, TY_STR], TY_BYTES, true),
            ("net_status", vec![TY_OPAQUE], TY_U64, false),
            ("net_error", vec![TY_OPAQUE], TY_STR, false),
            (
                "net_stream",
                vec![TY_U64, TY_STR, TY_STR, TY_STR],
                TY_OPAQUE,
                false,
            ),
            ("task_cancel", vec![TY_OPAQUE], TY_NIL, false),
        ]
        .into_iter()
        .map(|(n, p, r, a)| (n.to_string(), p, r, a)),
    );

    let Some(handles) = cx.handles else {
        return;
    };
    let handles = handles.clone();
    let pkg = &mut *cx.pkg;

    // net_request(url, method) -> bytes — the response body (raw bytes;
    // decode with `decode_utf8`). An error completes as empty bytes with
    // the message stashed on the task (net_status / net_error read it).
    let h = handles.clone();
    rut_vm::pkg_async_fn!(pkg, "net_request", (&str, &str) -> Vec<u8>, move |url: &str, method: &str| {
        let done = rut_vm::Completer::<Vec<u8>>::new();
        let Some(http) = h.inst.capability().of::<Http>() else {
            done.complete(Vec::new());
            return done;
        };
        let http = http.backend().clone();
        let opts = RequestOpts {
            url: url.to_string(),
            method: method.to_string(),
            headers: Vec::new(),
            body: None,
            stream_buffer_bytes: None,
        };
        let w = done.clone();
        h.inst.spawn_local(move |_aw| async move {
            match http.request(opts).await {
                HttpOutcome::Ok { body, .. } => w.complete(body),
                HttpOutcome::Err(_) => w.complete(Vec::new()),
            }
        });
        done
    });

    // net_status(task) -> u64 — the stashed HTTP status (0 = no response).
    rut_vm::pkg_fn!(pkg, "net_status", (Opaque<RutNetTask>,) -> u64, move |_vm: &mut rut_vm::interp::Vm, t: Opaque<RutNetTask>| {
        t.with(|t| Ok(*t.status.borrow() as u64))?
    });
    // net_error(task) -> str — the failure message ("" on success).
    rut_vm::pkg_fn!(pkg, "net_error", (Opaque<RutNetTask>,) -> String, move |_vm: &mut rut_vm::interp::Vm, t: Opaque<RutNetTask>| {
        t.with(|t| Ok(t.error.borrow().clone()))?
    });

    // net_stream(url, method, on_chunk) -> opaque — each chunk crosses as
    // an intent record (`entry fn cb(id: u64, data: bytes)`); the returned
    // task's `task_cancel` wire-aborts the download.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "net_stream", (u64, &str, &str, &str) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, id: u64, url: &str, method: &str, cb: &str| {
        let Some(http) = h.inst.capability().of::<Http>() else {
            return Err(rut_vm::Trap::new(rut_vm::TrapKind::Invalid, "no http capability"));
        };
        let http = http.backend().clone();
        let status: Rc<RefCell<u16>> = Rc::new(RefCell::new(0));
        let error: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let cb = cb.to_string();
        let h2 = h.clone();
        let (status2, error2) = (status.clone(), error.clone());
        let (url, method) = (url.to_string(), method.to_string());
        let handle = h.inst.spawn_local(move |_aw| async move {
            let opts = RequestOpts {
                url,
                method,
                headers: Vec::new(),
                body: None,
                stream_buffer_bytes: None,
            };
            let Ok(res) = http.request_stream(opts).await else {
                *error2.borrow_mut() = "stream failed".to_string();
                return;
            };
            *status2.borrow_mut() = res.status;
            let mut stream = res.body;
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        h2.pending_calls.borrow_mut().push(Intent::Bytes {
                            name: cb.clone(),
                            a: id,
                            data: bytes,
                        });
                        h2.dirty.set(true);
                    }
                    Err(e) => {
                        *error2.borrow_mut() = e;
                        return;
                    }
                }
            }
        });
        let task = RutNetTask {
            handle,
            status,
            error,
        };
        Ok(Opaque::alloc(vm, task)?.handle().clone())
    });

    // task_cancel(task) — aborts the spawned drive (a stream download
    // wire-aborts: the backend stream future drops mid-flight).
    rut_vm::pkg_fn!(pkg, "task_cancel", (Opaque<RutNetTask>,) -> (), move |_vm: &mut rut_vm::interp::Vm, t: Opaque<RutNetTask>| {
        t.with(|t| {
            t.handle.abort();
        })?;
        Ok(())
    });
}

// Re-exported for the harness-facing plugin below.
pub use plugin::TurRutNetRows;
/// The plugin that pushes the net rut rows (registered by embedders that
/// want the rut rail over HTTP; a no-op without the `Http` capability).
pub mod plugin {
    use super::*;

    pub struct TurRutNetRows;

    impl Default for TurRutNetRows {
        fn default() -> Self {
            Self
        }
    }

    impl tur_engine::core::plugin::Plugin for TurRutNetRows {
        fn register(
            &self,
            ctx: &mut tur_engine::core::plugin::PluginRegisterContext,
        ) -> Result<(), tur_engine::error::TurError> {
            if !ctx.capability().contains::<Http>() {
                tracing::info!("TurRutNetRows: no Http capability; skipping rut rows");
                return Ok(());
            }
            ctx.push_rut_ext(Rc::new(super::install));
            Ok(())
        }
    }
}

