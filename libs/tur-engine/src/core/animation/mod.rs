use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::object::JsObject;

use crate::core::animation::driver::{ImplicitDriver, TickOutcome};
use crate::core::animation::event::AnimationEndEvent;
use crate::core::edgy_event::{EdgyMutation, PendingMutationInvocationQueue};
use crate::core::element::ElementNodeId;
use crate::core::elements::NodeTree;
use tur_shared::Curve;

pub mod controller;
pub use controller::AnimationController;
pub mod driver;
pub mod event;
pub mod host;
pub mod props;

/// One entry in the implicit-animation driver registry. The manager owns the
/// timeline ([`ImplicitDriver`]); the element reads results via the shared
/// `eased_t` cell. `on_end` is enqueued on the mutation queue exactly once
/// when the timeline completes.
struct DriverEntry {
    element_id: ElementNodeId,
    driver: ImplicitDriver,
    eased_t: Rc<Cell<f64>>,
    on_end: Option<EdgyMutation<AnimationEndEvent>>,
}

#[derive(Default)]
pub struct AnimationManager {
    /// Explicit JS `AnimationController`s registered via `forward()`/`reverse()`.
    controllers: Vec<JsObject>,
    /// Native implicit-animation drivers, one per animated element. Reaped
    /// when the element is no longer in the tree (see [`Self::tick_drivers`]).
    drivers: Vec<DriverEntry>,
    /// Most recent wall-clock ms seen via [`Self::tick_drivers`] /
    /// [`Self::set_clock`]. Used by [`Self::retarget`] to stamp a driver's
    /// `start_time` immediately (so the timeline begins on the retarget
    /// frame, not the next one).
    now_ms: u64,
}

impl AnimationManager {
    pub fn new() -> Self {
        AnimationManager {
            controllers: Vec::new(),
            drivers: Vec::new(),
            now_ms: 0,
        }
    }

    // ----- explicit JS controllers (unchanged) ----------------------------

    pub fn register_controller(&mut self, obj: JsObject) {
        if !self.controllers.iter().any(|c| c == &obj) {
            self.controllers.push(obj);
        }
    }

    /// Tick all active JS controllers. Each tick updates `value` / `status` and
    /// **enqueues** (does not fire) any `onTick` / `onEnd` callbacks on the
    /// mutation queue. The callbacks fire later in `flush_pending_mutations`,
    /// after the `RefMut` on each controller is released.
    pub fn tick_controllers(&mut self, now_ms: u64, _ctx: &mut boa_engine::Context) {
        let mut active = Vec::new();
        for obj in self.controllers.drain(..) {
            let keep = {
                let Some(mut ctrl) = obj.downcast_mut::<AnimationController>() else {
                    continue;
                };
                let _ = ctrl.tick_compute(now_ms);
                ctrl.is_active()
            };
            if keep {
                active.push(obj);
            }
        }
        self.controllers = active;
    }

    // ----- native implicit-animation drivers ------------------------------

    /// Cache the latest wall-clock reading. Called from the frame loop before
    /// ticking so that [`Self::retarget`] (invoked later in the same flush,
    /// during the element Effect phase) can stamp a precise `start_time`.
    pub fn set_clock(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
    }

    /// Register (or look up) the driver for `element_id`. Returns the shared
    /// `eased_t` cell the element reads during layout. Called once per
    /// element on first effect.
    pub fn register(
        &mut self,
        element_id: ElementNodeId,
        duration_ms: u64,
        curve: Curve,
        on_end: Option<EdgyMutation<AnimationEndEvent>>,
    ) -> Rc<Cell<f64>> {
        if let Some(entry) = self
            .drivers
            .iter_mut()
            .find(|e| e.element_id == element_id)
        {
            // Re-registration: keep the existing eased_t cell so the element
            // retains its handle; refresh driver config.
            entry.driver = ImplicitDriver::new(duration_ms, curve);
            if on_end.is_some() {
                entry.on_end = on_end;
            }
            return Rc::clone(&entry.eased_t);
        }
        let eased_t = Rc::new(Cell::new(1.0));
        self.drivers.push(DriverEntry {
            element_id,
            driver: ImplicitDriver::new(duration_ms, curve),
            eased_t: Rc::clone(&eased_t),
            on_end,
        });
        eased_t
    }

    /// Request the driver for `element_id` restart from `t = 0` on its next
    /// tick. No-op if the element has no registered driver (e.g. no
    /// animatable prop change has occurred yet).
    ///
    /// Also resets the shared `eased_t` cell to `0.0` immediately so the
    /// current frame paints the *currently displayed* value (the new tween's
    /// `begin`) rather than jumping to the new target. Without this, the
    /// retarget frame would read the stale `eased_t = 1.0` and snap.
    pub fn retarget(&mut self, element_id: ElementNodeId) {
        if let Some(entry) = self
            .drivers
            .iter_mut()
            .find(|e| e.element_id == element_id)
        {
            // Stamp the start time immediately using the cached clock so the
            // timeline begins advancing on this very frame (the next
            // `tick_drivers` measures elapsed from `now_ms`).
            entry.driver.start_at(self.now_ms);
            entry.eased_t.set(0.0);
        }
    }

    /// Advance every active driver by the elapsed wall-clock. For each tick
    /// that produced a new eased `t`, write it into the shared cell and mark
    /// the element dirty so its `perform_layout` re-runs. Enqueue `onEnd`
    /// (once) on completion. Reap entries whose element has left the tree.
    ///
    /// Returns `true` if at least one driver ticked (so the caller keeps the
    /// frame loop alive).
    pub fn tick_drivers(
        &mut self,
        now_ms: u64,
        element_tree: &NodeTree,
        dirty: &Rc<Cell<bool>>,
        mutation_queue: &Rc<RefCell<PendingMutationInvocationQueue>>,
    ) -> bool {
        if self.drivers.is_empty() {
            return false;
        }
        let mut any_ticked = false;
        let mut on_end_fires: Vec<EdgyMutation<AnimationEndEvent>> = Vec::new();

        // Drain + filter + tick in one pass. Dead entries (element gone) are
        // dropped; alive entries are collected back. Marking dirty + reading
        // element existence borrow `NodeTree` (a separate Rc<RefCell>), which
        // does not conflict with our `&mut self` borrow.
        let drivers = std::mem::take(&mut self.drivers);
        let mut alive = Vec::with_capacity(drivers.len());
        for mut entry in drivers {
            // Reap entries whose element has unmounted.
            if element_tree.get_element(entry.element_id).is_none() {
                continue;
            }
            if let Some(TickOutcome { eased_t, just_completed }) = entry.driver.tick(now_ms) {
                entry.eased_t.set(eased_t);
                element_tree.mark_dirty(entry.element_id.into());
                dirty.set(true);
                any_ticked = true;
                if just_completed && entry.on_end.is_some() {
                    on_end_fires.push(entry.on_end.take().unwrap());
                }
            }
            alive.push(entry);
        }
        self.drivers = alive;

        // Fire onEnd callbacks via the mutation queue (deferred, like the JS
        // controller path — runs in flush_pending_mutations after borrows
        // release).
        if !on_end_fires.is_empty() {
            let mut q = mutation_queue.borrow_mut();
            for m in on_end_fires {
                q.push(m, AnimationEndEvent);
            }
        }

        any_ticked
    }

    pub fn has_active(&self) -> bool {
        !self.controllers.is_empty() || self.drivers.iter().any(|e| e.driver.is_active())
    }
}

// `EdgyMutation` has no `Debug` impl, so `DriverEntry` (and hence
// `AnimationManager`) can't derive `Debug`. `TurJsContext` derives `Debug` and
// holds the manager, so provide a manual impl.
impl std::fmt::Debug for AnimationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimationManager")
            .field("controllers", &self.controllers.len())
            .field("drivers", &self.drivers.len())
            .finish()
    }
}
