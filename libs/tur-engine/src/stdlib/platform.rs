//! Platform capability traits for the std widget library's plugins.
//!
//! These describe host services the engine can't provide itself (cursor
//! output). Clipboard moved to `tur-clipboard-capability`; HTTP lives in
//! `tur-net`. The traits here are registered as capability newtypes via
//! [`crate::TurEngineBuilder::capability`] and looked up at runtime via the
//! [`crate::core::capability::Capabilities`] registry.

use std::cell::RefCell;
use std::rc::Rc;

use tur_shared::Cursor;

use crate::core::capability::Capability;

/// Cursor output capability. The engine pushes the resolved cursor (deepest
/// painted `MouseRegion` claim) during the frame loop's `apply_changes`
/// pass; the backend applies it (e.g. set the host canvas CSS cursor in
/// tur-wasm).
///
/// Backends are registered via
/// [`crate::TurEngineBuilder::capability`](`CursorCap::new(...)`); the engine
/// builder looks the cap up after all plugins register and installs the
/// backend on [`crate::core::shell::Shell`]. If absent, the engine falls
/// back to [`NoopCursor`] (cursor never changes).
pub trait CursorBackend {
    fn set_cursor(&mut self, cursor: Cursor);
}

/// No-op `CursorBackend` default.
pub struct NoopCursor;
impl CursorBackend for NoopCursor {
    fn set_cursor(&mut self, _cursor: Cursor) {}
}

/// Capability newtype wrapping a `Rc<RefCell<dyn CursorBackend>>`. The
/// `RefCell` is required because [`CursorBackend::set_cursor`] takes
/// `&mut self` but the engine holds the backend in a shared `Rc` (cloned
/// across the registry).
///
/// Named `CursorCap` (not `Cursor`) because `tur_shared::Cursor` already
/// names the cursor-kind enum (`Default`, `Pointer`, ...) used pervasively
/// across the engine — renaming that would be ~74 mechanical call-site
/// edits for a naming preference. This is the lone exception to the
/// "capability newtypes use base names" convention.
///
/// Registered via [`crate::TurEngineBuilder::capability`] with
/// `CursorCap::new(backend)`; the engine builder installs the backend on the
/// Shell at `build()` time.
#[derive(Clone)]
pub struct CursorCap(Rc<RefCell<dyn CursorBackend>>);

impl CursorCap {
    /// Wrap a backend in the capability newtype.
    pub fn new(backend: impl CursorBackend + 'static) -> Self {
        Self(Rc::new(RefCell::new(backend)))
    }

    /// Borrow the underlying backend handle.
    pub fn backend(&self) -> &Rc<RefCell<dyn CursorBackend>> {
        &self.0
    }
}

impl Capability for CursorCap {}
