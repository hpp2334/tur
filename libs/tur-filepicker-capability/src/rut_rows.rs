//! C6 — the filepicker rut rows (via the pkg-extension seam):
//! `pick_file` (async — the picked file's name, "" when cancelled). The
//! async start lane cannot mint opaques (no `&mut Vm` there), so the name
//! is the crossing; the bytes ride with the sync task-row milestone.

use std::rc::Rc;

use rut_core::types::{TY_BOOL, TY_BYTES, TY_STR};

use crate::{FilePicker, PickOptions, SaveOptions};

/// Declare the rows + install the bodies (the pkg-extension payload).
/// (The allow covers the upstream `pkg_async_fn!` row expansion's
/// cosmetic — the spike's precedent.)
#[allow(clippy::needless_question_mark)]
pub fn install(cx: &mut tur_engine::core::rut_runtime::RutPkgCx<'_>) {
    // The file kit — the authored face over these rows (the plain public
    // names live there; the rows carry the `_raw` twin spelling, the
    // `mount_raw` pattern). Registered alongside the rows, so it compiles
    // only when the FilePicker capability is present.
    cx.preludes.push(crate::kit::file_kit_pkg());
    cx.decl.extend(vec![
        tur_engine::core::rut_runtime::host_row_at(
            "filepicker",
            "pick_file_raw",
            vec![],
            TY_STR,
            true,
        ),
        tur_engine::core::rut_runtime::host_row_at(
            "filepicker",
            "save_file_raw",
            vec![TY_STR, TY_BYTES],
            TY_BOOL,
            true,
        ),
    ]);

    let Some(handles) = cx.handles else {
        return;
    };
    let handles = handles.clone();
    let pkg = &mut *cx.pkg;

    // pick_file_raw() -> str — the first picked file's name ("" when
    // cancelled/denied, matching the JS bridge's empty-Vec degradation).
    // The kit's `pick_file` wraps it.
    let h = handles.clone();
    rut_vm::pkg_async_fn!(pkg, "filepicker::pick_file_raw", () -> String, move || {
        let done = rut_vm::Completer::<String>::new();
        let Some(picker) = h.inst.capability().of::<FilePicker>() else {
            done.complete(String::new());
            return done;
        };
        let picker = picker.backend().clone();
        let w = done.clone();
        h.inst.spawn_local(move |_aw| async move {
            let picked = picker
                .pick(PickOptions {
                    accept: Vec::new(),
                    multiple: false,
                })
                .await;
            w.complete(
                picked
                    .into_iter()
                    .next()
                    .map(|f| f.name)
                    .unwrap_or_default(),
            );
        });
        done
    });

    // save_file_raw(name, bytes) -> bool — persist `bytes` under `name`
    // through the platform save lane (the browser's download dialog on
    // wasm, the rfd save dialog on native). Resolves false when the
    // capability is absent ("Save not available in this host") — the
    // caller's degradation signal. The kit's `save_file` wraps it.
    let h = handles.clone();
    rut_vm::pkg_async_fn!(pkg, "filepicker::save_file_raw", (&str, Vec<u8>) -> bool, move |name: &str, bytes: Vec<u8>| {
        let done = rut_vm::Completer::<bool>::new();
        let Some(picker) = h.inst.capability().of::<FilePicker>() else {
            done.complete(false);
            return done;
        };
        let picker = picker.backend().clone();
        let name = name.to_string();
        let w = done.clone();
        h.inst.spawn_local(move |_aw| async move {
            picker.save(name, bytes, SaveOptions::default()).await;
            w.complete(true);
        });
        done
    });
}

/// The plugin that pushes the filepicker rut rows (registered by embedders
/// that want the rut rail over the file picker; a no-op without the
/// `FilePicker` capability).
pub mod plugin {
    use super::*;

    pub struct TurRutFilePickerRows;

    impl Default for TurRutFilePickerRows {
        fn default() -> Self {
            Self
        }
    }

    impl tur_engine::core::plugin::Plugin for TurRutFilePickerRows {
        fn register(
            &self,
            ctx: &mut tur_engine::core::plugin::PluginRegisterContext,
        ) -> Result<(), tur_engine::error::TurError> {
            if !ctx.capability().contains::<FilePicker>() {
                tracing::info!("TurRutFilePickerRows: no FilePicker capability; skipping rut rows");
                return Ok(());
            }
            ctx.push_rut_ext(Rc::new(super::install));
            Ok(())
        }
    }
}
