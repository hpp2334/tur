//! Native file-picker backend for tur, backed by the [`rfd`] crate
//! (cross-platform async file dialog: win/mac/linux).
//!
//! Re-exports the file-picker plugin surface from [`tur_filepicker_capability`]
//! so native embedders only need this one crate. The backend
//! ([`NativeFilePicker`]) is registered via
//! `TurRuntimeBuilder::capability(FilePicker::new(NativeFilePicker::default()))`.
//!
//! On wasm this crate compiles as a near-empty stub (the `rfd` dep is
//! target-gated to `cfg(not(target_family = "wasm"))`). Embedders targeting
//! wasm should depend on `tur-filepicker-wasm` instead.

pub use tur_filepicker_capability::{
    FilePicker, FilePickerBackend, PickOptions, PickedFile, SaveOptions, TurFilePickerPlugin,
};

#[cfg(not(target_family = "wasm"))]
mod backend;

#[cfg(not(target_family = "wasm"))]
pub use backend::NativeFilePicker;
