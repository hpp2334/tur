//! Clipboard capability surface — backend trait + capability newtype.
//!
//! These are the **public** types of the clipboard plugin: external backend
//! crates (`tur-clipboard-wasm`, `tur-clipboard-native`,
//! `tur-integration-tests::RecordingClipboard`) implement
//! [`ClipboardBackend`], and embedders construct the [`Clipboard`] capability
//! newtype via `Clipboard::new(backend)` to register on
//! [`TurRuntimeBuilder::capability`](crate::TurRuntimeBuilder::capability).
//!
//! Both types are re-exported at the engine crate root
//! (`tur_engine::Clipboard`, `tur_engine::ClipboardBackend`) so external
//! consumers don't need to reach into `builtin_plugins::clipboard`.

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use crate::core::capability::Capability;

/// Async clipboard backend. Methods return boxed `Future`s because the
/// trait is held as `dyn ClipboardBackend` (object-safe). Backends decide
/// whether the operation is actually async — sync backends (e.g. a test
/// stub) return `std::future::ready(...)`.
///
/// Backends are registered as the [`Clipboard`] capability via
/// `tur_engine::TurRuntimeBuilder::capability(Clipboard::new(backend))`.
/// Bridge fns look up the cap at JS call time and call these methods.
///
/// On wasm, `navigator.clipboard.readText/writeText` are inherently async
/// (return JS Promises); on native/tests, this can resolve eagerly.
pub trait ClipboardBackend: 'static {
    /// Read text from the clipboard. Resolves with the text (empty string
    /// if denied/unavailable).
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>>;

    /// Write text to the clipboard. Resolves when the write has been
    /// acknowledged by the platform.
    fn write_text(&self, text: String) -> Pin<Box<dyn Future<Output = ()>>>;
}

/// Capability newtype wrapping an `Rc<dyn ClipboardBackend>`. Registered via
/// [`tur_engine::TurRuntimeBuilder::capability`] with
/// `Clipboard::new(backend)`; bridge fns look it up at JS call time via
/// `js_ctx.capability().of::<Clipboard>()`.
///
/// [`tur_engine::TurRuntimeBuilder::capability`]: crate::TurRuntimeBuilder::capability
#[derive(Clone)]
pub struct Clipboard(Rc<dyn ClipboardBackend>);

impl Clipboard {
    /// Wrap a backend in the capability newtype.
    pub fn new(backend: impl ClipboardBackend + 'static) -> Self {
        Self(Rc::new(backend))
    }

    /// Borrow the underlying backend handle.
    pub fn backend(&self) -> &Rc<dyn ClipboardBackend> {
        &self.0
    }
}

impl Capability for Clipboard {}
