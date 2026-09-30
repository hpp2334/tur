//! C6 — the filepicker rut rows (via the pkg-extension seam):
//! `pick_file` (async — the picked file's name, "" when cancelled). The
//! async start lane cannot mint opaques (no `&mut Vm` there), so the name
//! is the crossing; the bytes ride with the sync task-row milestone.

use std::rc::Rc;

use rut_core::types::TY_STR;

use crate::{FilePicker, PickOptions};

/// Declare the rows + install the bodies (the pkg-extension payload).
/// (The allow covers the upstream `pkg_async_fn!` row expansion's
/// cosmetic — the spike's precedent.)
#[allow(clippy::needless_question_mark)]
pub fn install(cx: &mut tur_engine::core::rut_runtime::RutPkgCx<'_>) {
    cx.decl.extend(vec![("pick_file".to_string(), vec![], TY_STR, true)]);

    let Some(handles) = cx.handles else {
        return;
    };
    let handles = handles.clone();
    let pkg = &mut *cx.pkg;

    // pick_file() -> str — the first picked file's name ("" when
    // cancelled/denied, matching the JS bridge's empty-Vec degradation).
    let h = handles.clone();
    rut_vm::pkg_async_fn!(pkg, "pick_file", () -> String, move || {
        let done = rut_vm::Completer::<String>::new();
        let Some(picker) = h.js_ctx.capability().of::<FilePicker>() else {
            done.complete(String::new());
            return done;
        };
        let picker = picker.backend().clone();
        let w = done.clone();
        h.js_ctx.spawn_local(move |_aw| async move {
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
            ctx: &mut tur_engine::core::plugin::PluginRegisterContext<'_>,
        ) -> Result<(), tur_engine::error::TurError> {
            if !ctx.js_ctx().capability().contains::<FilePicker>() {
                tracing::info!("TurRutFilePickerRows: no FilePicker capability; skipping rut rows");
                return Ok(());
            }
            ctx.js_ctx()
                .rut_pkg_exts
                .borrow_mut()
                .push(Rc::new(super::install));
            Ok(())
        }
    }
}
