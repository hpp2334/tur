//! File picker capability for tur.
//!
//! Provides:
//!
//! - The [`FilePickerBackend`] capability trait + supporting types
//!   ([`PickOptions`], [`SaveOptions`], [`PickedFile`]).
//! - The [`FilePicker`] capability newtype wrapping `Rc<dyn FilePickerBackend>`,
//!   registered via
//!   [`tur_engine::TurRuntimeBuilder::capability`](`FilePicker::new(backend)`).
//! - The [`TurFilePickerPlugin`] (unit struct) that registers the
//!   `tur:filepicker` module (exporting a single `filePicker` object with
//!   `pick` / `saveFile` methods). The plugin declares a hard `requires` on
//!   [`FilePicker`] — the builder fails fast at `build()` if no backend is
//!   registered.
//!
//! ## Architecture
//!
//! - Backends (`WasmFilePicker` in `tur-filepicker-wasm`, `NativeFilePicker`
//!   in `tur-filepicker-native`, `RecordingFilePicker` in
//!   `tur-integration-tests`) implement [`FilePickerBackend`] and are
//!   registered via `.capability(FilePicker::new(backend))`.
//! - The bridge (in [`bridge`]) parses JS opts into [`PickOptions`] /
//!   [`SaveOptions`], spawns the future via the engine's `AsyncExecutor`, and
//!   settles the `JsPromise` via a completion closure on the next `flush`.
//! - File picking is **opt-in**: unlike `tur:net` (an optional capability that
//!   silently skips when absent), [`TurFilePickerPlugin`] declares
//!   `requires(FilePicker)` and fails fast — hosts that want `tur:filepicker`
//!   must register a real backend. There is intentionally no no-op default:
//!   code that imports `tur:filepicker` without the plugin installed crashes
//!   loudly rather than silently doing nothing.

pub mod bridge;

use std::future::Future;
use std::pin::Pin;

use tur_engine::core::capability::CapabilityDecls;
use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::error::TurError;

// ---------------------------------------------------------------------------
// FilePicker capability trait + supporting types
// ---------------------------------------------------------------------------

/// A picked file: its file name + raw bytes + optional MIME type (when the
/// platform reports one). The bridge builds the JS
/// `{ name, bytes: ArrayBuffer, type, size }` object from this.
#[derive(Debug, Clone)]
pub struct PickedFile {
    /// File name (no path). The platform-supplied leaf name.
    pub name: String,
    /// Raw file bytes, copied into a JS `ArrayBuffer` by the bridge.
    pub bytes: Vec<u8>,
    /// MIME type when the platform reports one (e.g. `"image/png"`); empty
    /// string otherwise.
    pub mime_type: Option<String>,
}

/// Options parsed from the JS `pick(opts)` object:
/// `{ accept?: string[], multiple?: boolean }`.
#[derive(Debug, Clone, Default)]
pub struct PickOptions {
    /// Accepted file filters — MIME types (`"image/*"`) or extensions
    /// (`".png"`). Platform-dependent how each is honored (browsers accept
    /// both via `<input accept>`; native `rfd` derives extensions only).
    pub accept: Vec<String>,
    /// Allow selecting more than one file. `pick` always resolves with a
    /// `Vec`; `multiple = false` yields at most one entry.
    pub multiple: bool,
}

/// Options parsed from the JS `saveFile(name, bytes, opts)` object:
/// `{ accept?: string[] }`.
#[derive(Debug, Clone, Default)]
pub struct SaveOptions {
    /// Suggested save filters (MIME/extension). Platform-dependent.
    pub accept: Vec<String>,
}

/// Async file-picker backend. Backends provide an impl (`WasmFilePicker` via
/// web-sys on wasm; `NativeFilePicker` via `rfd` on native; `RecordingFilePicker`
/// for tests) and register it via
/// `TurRuntimeBuilder::capability(FilePicker::new(backend))`. The bridge fns
/// `pick` / `saveFile` in `tur:filepicker` consume it.
pub trait FilePickerBackend: Send + Sync + 'static {
    /// Open the platform file picker. Resolves with the picked files (empty
    /// `Vec` if cancelled/denied).
    fn pick(&self, opts: PickOptions) -> Pin<Box<dyn Future<Output = Vec<PickedFile>>>>;

    /// Persist `bytes` under file name `name` (and the platform save dialog).
    /// Resolves once the write has been acknowledged (or the platform
    /// download has been triggered).
    fn save(
        &self,
        name: String,
        bytes: Vec<u8>,
        opts: SaveOptions,
    ) -> Pin<Box<dyn Future<Output = ()>>>;
}

/// Capability newtype wrapping an `Rc<dyn FilePickerBackend>`. Registered via
/// [`tur_engine::TurRuntimeBuilder::capability`] with `FilePicker::new(backend)`;
/// the bridge fns look it up at call time via
/// `js_ctx.capability().of::<FilePicker>()`.
#[derive(Clone)]
pub struct FilePicker(std::sync::Arc<dyn FilePickerBackend + Send + Sync>);

impl FilePicker {
    /// Wrap a backend in the capability newtype.
    pub fn new(backend: impl FilePickerBackend + 'static) -> Self {
        Self(std::sync::Arc::new(backend))
    }

    /// Borrow the underlying backend handle.
    pub fn backend(&self) -> &std::sync::Arc<dyn FilePickerBackend + Send + Sync> {
        &self.0
    }
}

impl tur_engine::core::capability::Capability for FilePicker {}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// tur-filepicker plugin: registers `tur:filepicker`, exporting a single
/// `filePicker` object with `pick(opts?)` / `saveFile(name, bytes, opts?)`
/// methods (each returning a `Task` — `{ promise, cancel() }`).
///
/// The plugin declares a hard dependency on the [`FilePicker`] capability via
/// `requires`; the engine builder fails fast at `build()` if the embedder
/// forgot to register a backend via `.capability(FilePicker::new(...))`.
///
/// The bridge fns are ctx-bound `Ptr`s that read their `FilePicker` +
/// `AsyncExecutor` from `TurInstanceContext`'s capability registry at call time — no
/// `unsafe NativeFunction::from_closure` (see [`bridge`]).
pub struct TurFilePickerPlugin;

impl Default for TurFilePickerPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurFilePickerPlugin {
    fn requires(&self, decls: &mut CapabilityDecls) {
        decls.need::<FilePicker>();
    }

    fn register(&self, ctx: &mut PluginRegisterContext<'_>) -> Result<(), TurError> {
        let ctx_value = ctx.js_ctx_value.clone();
        let filepicker_obj = bridge::build_filepicker_object(ctx.boa_mut(), ctx_value);
        let consts: Vec<tur_engine::core::js_runtime::helpers::ConstEntry> =
            vec![("filePicker", filepicker_obj)];
        ctx.register_module("tur:filepicker", bridge::fns(), consts);
        Ok(())
    }
}
