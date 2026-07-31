//! `FilePickerBackend` impl backed by the `rfd` crate's `AsyncFileDialog`.
//! `rfd` runs the dialog off the calling thread (macOS AppKit / GTK / Win32)
//! and resolves via an async future, so the engine frame loop isn't blocked.

use std::future::Future;
use std::pin::Pin;

use rfd::AsyncFileDialog;

use tur_filepicker_capability::{FilePickerBackend, PickOptions, PickedFile, SaveOptions};

/// Native file-picker backend. A unit struct — `rfd` dialogs are created
/// per-call, so there's no handle to hold.
#[derive(Default)]
pub struct NativeFilePicker;

/// Derive rfd extension filters from the `accept` list. Only entries shaped
/// like extensions (`.png`, `.txt`) map; MIME/glob entries (`image/*`) are
/// dropped — `rfd` needs concrete extensions. All extension entries are merged
/// into a single filter.
fn rfd_extensions(accept: &[String]) -> Vec<String> {
    accept
        .iter()
        .filter_map(|a| {
            a.strip_prefix('.')
                .map(|s| s.to_ascii_lowercase())
                .filter(|s| !s.is_empty())
        })
        .collect()
}

/// Best-effort MIME guess from a file extension for the JS `type` field.
/// Unknown extensions yield `None` (empty string in JS).
fn guess_mime(path: &std::path::Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "html" => "text/html",
        "css" => "text/css",
        "txt" | "md" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "pdf" => "application/pdf",
        _ => return None,
    };
    Some(mime.to_string())
}

impl FilePickerBackend for NativeFilePicker {
    fn pick(&self, opts: PickOptions) -> Pin<Box<dyn Future<Output = Vec<PickedFile>>>> {
        Box::pin(async move {
            let mut dlg = AsyncFileDialog::new();
            let exts = rfd_extensions(&opts.accept);
            if !exts.is_empty() {
                dlg = dlg.add_filter("Allowed", &exts);
            }
            // `pick_files` always allows multiple selection; for single mode
            // use `pick_file` so the dialog enforces one.
            let handles = if opts.multiple {
                dlg.pick_files().await.unwrap_or_default()
            } else {
                dlg.pick_file().await.into_iter().collect::<Vec<_>>()
            };

            let mut picked = Vec::with_capacity(handles.len());
            for h in handles {
                let path = h.path();
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                match std::fs::read(path) {
                    Ok(bytes) => picked.push(PickedFile {
                        name,
                        bytes,
                        mime_type: guess_mime(path),
                    }),
                    Err(e) => tracing::warn!("filepicker read {:?} failed: {e}", path),
                }
            }
            picked
        })
    }

    fn save(
        &self,
        name: String,
        bytes: Vec<u8>,
        opts: SaveOptions,
    ) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            let mut dlg = AsyncFileDialog::new().set_file_name(&name);
            let exts = rfd_extensions(&opts.accept);
            if !exts.is_empty() {
                dlg = dlg.add_filter("Allowed", &exts);
            }
            if let Some(handle) = dlg.save_file().await {
                let path = handle.path().to_path_buf();
                if let Err(e) = std::fs::write(&path, &bytes) {
                    tracing::warn!("filepicker write {:?} failed: {e}", path);
                }
            }
            // else: cancelled — resolve without writing.
        })
    }
}
