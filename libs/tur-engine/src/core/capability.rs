//! Capability registry: a type-keyed service locator for plugin-swappable
//! backends.
//!
//! A [`Capability`] is a marker trait implemented by cheaply-clonable newtype
//! wrappers around `Rc<dyn BackendTrait>` (e.g. `Clipboard(Rc<dyn ClipboardBackend>)`).
//! Each capability type is stored under its `TypeId` in a shared map held by
//! [`crate::core::js_runtime::TurJsContext`], and looked up by:
//!
//! - **Bridge fns** via `js_ctx.capability().of::<C>()` (at JS call time).
//! - **Handlers** via `cx.capabilities.of::<C>()` (at event dispatch time).
//! - **The engine builder** internally (e.g. for the `Cursor` install on
//!   [`crate::core::shell::Shell`]).
//!
//! Plugins declare their hard dependencies via [`Plugin::requires`] so the
//! engine builder can verify all required capabilities are registered before
//! any plugin's `register` runs — missing capabilities cause `build()` to
//! return `TurError::Other` with a clear message naming the missing type and
//! the fix, instead of failing midway through side-effecting registration.
//!
//! [`Plugin::requires`]: crate::core::plugin::Plugin::requires

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::error::TurError;

/// Marker trait gating what can be registered as a capability.
///
/// Newtype capability wrappers implement this explicitly
/// (`impl Capability for Clipboard {}`). The `Any + Clone + 'static` bound is
/// the registry mechanics (storage as `Box<dyn Any>`, lookup via `downcast` +
/// `clone`); requiring an explicit `impl` prevents accidental registration of
/// arbitrary types.
///
/// Convention: capability newtypes are named after the service they wrap
/// (`Clipboard`, `Http`, `Cursor`), with the backend trait suffixed `Backend`
/// (`ClipboardBackend`, `HttpBackend`, `CursorBackend`).
pub trait Capability: Any + Clone + 'static {}

/// Cheaply-cloned view over the type-erased capability registry.
///
/// Cloning a `Capabilities` clones only the inner `Rc` — all clones share the
/// same backing map. Returned by [`crate::core::js_runtime::TurJsContext::capability`]
/// and [`crate::core::plugin::PluginContext::capability`]; also held by
/// [`crate::core::handler::HandlerContext`] so event handlers can look up
/// capabilities at dispatch time.
#[derive(Clone, Debug)]
pub struct Capabilities {
    map: Rc<RefCell<HashMap<TypeId, Box<dyn Any>>>>,
}

impl Capabilities {
    pub fn new() -> Self {
        Self {
            map: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Insert (or overwrite) a capability under its `TypeId`. Plugins call
    /// this from `register` to publish a capability to later plugins / bridge
    /// fns / handlers.
    pub fn insert<C: Capability>(&self, cap: C) {
        self.map
            .borrow_mut()
            .insert(TypeId::of::<C>(), Box::new(cap));
    }

    /// Read a capability by type. Returns a clone of the stored newtype (which
    /// is itself cheaply-clonable — typically a single `Rc` bump).
    pub fn of<C: Capability>(&self) -> Option<C> {
        self.map
            .borrow()
            .get(&TypeId::of::<C>())
            .and_then(|c| c.downcast_ref::<C>())
            .cloned()
    }

    /// Read a capability or return a descriptive error. Used by code paths
    /// where absence is a hard error (e.g. a bridge fn whose plugin declared
    /// a `requires` that should already have been validated at `build()`).
    pub fn require<C: Capability>(&self) -> Result<C, TurError> {
        self.of::<C>().ok_or_else(|| {
            TurError::Other(format!(
                "capability `{}` not registered",
                std::any::type_name::<C>()
            ))
        })
    }

    /// True if a capability of type `C` is registered.
    pub fn contains<C: Capability>(&self) -> bool {
        self.map.borrow().contains_key(&TypeId::of::<C>())
    }

    /// True if a capability with the given `TypeId` is registered. Used by
    /// the engine builder's `requires` validation, which works with `TypeId`s
    /// collected from [`Plugin::requires`].
    pub(crate) fn contains_id(&self, id: &TypeId) -> bool {
        self.map.borrow().contains_key(id)
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// Collector passed to [`Plugin::requires`](crate::core::plugin::Plugin::requires).
/// Plugins call `decls.need::<C>()` for each capability they hard-require.
///
/// Collected declarations are validated by the engine builder before any
/// plugin's `register` runs, so missing dependencies fail fast at `build()`.
pub struct CapabilityDecls {
    inner: Vec<Decl>,
}

struct Decl {
    type_id: TypeId,
    type_name: &'static str,
}

impl CapabilityDecls {
    pub(crate) fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Declare that the calling plugin requires capability `C`. The engine
    /// verifies `C` is registered (via
    /// [`crate::TurRuntimeBuilder::capability`]) before calling the plugin's
    /// `register`.
    ///
    /// Optional dependencies should NOT be declared here — the plugin should
    /// look them up via `ctx.capability().of::<C>()` in `register` and handle
    /// absence gracefully (see `TurNetPlugin` for the "skip module
    /// registration if Http is absent" pattern).
    pub fn need<C: Capability>(&mut self) {
        self.inner.push(Decl {
            type_id: TypeId::of::<C>(),
            type_name: std::any::type_name::<C>(),
        });
    }

    /// Iterate over the declared (TypeId, type-name) pairs. Used by the
    /// engine builder's validation pass.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&TypeId, &'static str)> {
        self.inner.iter().map(|d| (&d.type_id, d.type_name))
    }
}

impl Default for CapabilityDecls {
    fn default() -> Self {
        Self::new()
    }
}
