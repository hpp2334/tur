//! Browser file-picker backend for tur, backed by a hidden
//! `<input type="file">` (pick) and a Blob + `<a download>` (save).
//!
//! Re-exports the file-picker plugin surface from [`tur_filepicker_capability`]
//! so browser embedders only need this one crate. The backend
//! ([`WasmFilePicker`]) is registered via
//! `TurEngineBuilder::capability(FilePicker::new(WasmFilePicker))`.

mod backend;

pub use backend::WasmFilePicker;
pub use tur_filepicker_capability::{
    FilePicker, FilePickerBackend, PickOptions, PickedFile, SaveOptions, TurFilePickerPlugin,
};
