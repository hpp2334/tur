//! The rut rail's realm face — the coexistence-era bridge to the JS realm.
//!
//! The migration's END GOAL is full boa removal, but Phase C's rows need
//! JS-class-backed state while both rails live: a rut `TextEditingController`
//! (C1) must be the SAME object the engine's `EditableTextElement` reads and
//! writes, and that state is a boa `JsData` payload. Rut rows cannot mint a
//! `JsObject` themselves (they hold `&mut Vm`, never `&mut Context`), so this
//! face gives them a strictly-scoped borrow: `with_realm(|boa| …)`.
//!
//! Design laws:
//! - **One home.** The realm storage is the engine's own
//!   `WorkerBackend::realm` slot (`Rc<RealmSlot>`); this face holds a clone —
//!   it never constructs a second `Context`.
//! - **Lazy construction.** `with_realm` ensures the realm exists first
//!   (the same deferred-registration replay a JS module load runs), so a rut
//!   row demanding the realm is indistinguishable from a JS load as far as
//!   the plugin surface is concerned.
//! - **No cycles.** The constructor closure captures cheap clones only
//!   (`Rc`s over engine state), never the `WorkerBackend`.
//! - **Scoped borrows.** The rut VM always runs with the realm borrow
//!   released (the pump's drain-first discipline), so a row's `with_realm`
//!   can never race a flush. Nested `with_realm` inside one row call would
//!   double-borrow — rows borrow once per call site.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::Context;

use crate::core::runtime::backend::RealmSlot;

/// The realm constructor closure (cheap clones over engine state only).
type RealmEnsure = Rc<dyn Fn() -> Result<(), String>>;

/// The per-instance realm face handed to rut rows via [`RutHandles`]
/// (see `super::RutHandles`). Cloneable (an `Rc` over the shared slot).
#[derive(Clone)]
pub struct RutRealm {
    slot: Option<Rc<RealmSlot>>,
    /// The realm constructor, installed by the worker backend at boot
    /// (`WorkerBackend::arm_realm_face`). `Rc` so the installed closure can
    /// be invoked through a shared borrow.
    ensure: RefCell<Option<RealmEnsure>>,
}

impl RutRealm {
    /// A face with no slot yet — replaced by [`Self::attach`] at boot.
    pub(crate) fn detached() -> Self {
        Self {
            slot: None,
            ensure: RefCell::new(None),
        }
    }

    /// Bind the shared slot + constructor (boot path).
    pub(crate) fn arm(&mut self, slot: Rc<RealmSlot>, ensure: RealmEnsure) {
        self.slot = Some(slot);
        *self.ensure.borrow_mut() = Some(ensure);
    }

    /// Run `f` with the realm, constructing it first when absent. The
    /// borrow is held for exactly the call — rows must not nest.
    pub fn with_realm<T>(&self, f: impl FnOnce(&mut Context) -> T) -> Result<T, String> {
        let slot = self
            .slot
            .as_ref()
            .ok_or_else(|| "realm face not attached (no worker backend)".to_string())?;
        if slot.realm.borrow().is_none() {
            let ensure = self
                .ensure
                .borrow()
                .clone()
                .ok_or_else(|| "realm face not armed (no worker backend)".to_string())?;
            ensure()?;
        }
        let mut guard = slot.realm.borrow_mut();
        let boa = guard
            .as_mut()
            .ok_or_else(|| "realm absent after ensure".to_string())?;
        Ok(f(boa))
    }

    /// Whether the realm currently exists (no construction).
    pub fn allocated(&self) -> bool {
        self.slot.as_ref().is_some_and(|s| s.realm.borrow().is_some())
    }
}
