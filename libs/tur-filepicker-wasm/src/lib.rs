//! Browser file-picker backend for tur, backed by a hidden
//! `<input type="file">` (pick) and a Blob + `<a download>` (save).
//!
//! Re-exports the file-picker plugin surface from [`tur_filepicker_capability`]
//! so browser embedders only need this one crate. The backend
//! ([`WasmFilePicker`]) is registered via
//! `TurRuntimeBuilder::capability(FilePicker::new(WasmFilePicker))`.

mod backend;
mod bridge;

pub use backend::WasmFilePicker;
pub use bridge::{route_worker_message, serve_host_message};
pub use tur_filepicker_capability::{
    FilePicker, FilePickerBackend, PickOptions, PickedFile, SaveOptions, TurFilePickerPlugin,
};
