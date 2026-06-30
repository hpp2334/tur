//! `ImplicitAnimationHost` — the per-element handle into the implicit-animation
//! machinery. Embedded in each animated element (`AnimatedContainerElement`,
//! `AnimatedOpacityElement`, `AnimatedPositionedElement`).
//!
//! Responsibilities:
//!   - Lazily registers a driver entry with [`super::AnimationManager`] on
//!     first use (creating the shared `eased_t` cell the element reads during
//!     layout).
//!   - Forwards retarget requests when the element detects a target change.
//!   - Exposes the latest eased `t` for the element's `perform_layout`.
//!
//! The element owns per-prop tween state separately
//! ([`super::props::AnimatedProp`]); the host only owns the timeline handle.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use tur_shared::Curve;

use crate::core::animation::event::AnimationEndEvent;
use crate::core::animation::AnimationManager;
use crate::core::element::ElementNodeId;
use crate::core::edgy_event::EdgyMutation;
use crate::core::view::SharedViewCx;

/// Per-element animation handle. `eased_t` defaults to `1.0` (settled) so a
/// freshly-mounted element paints its targets immediately, before the first
/// tick — matching Flutter's first-frame rule.
#[derive(Clone, Default)]
pub struct ImplicitAnimationHost {
    eased_t: Option<Rc<Cell<f64>>>,
    on_end: Option<EdgyMutation<AnimationEndEvent>>,
    registered: bool,
}

impl ImplicitAnimationHost {
    pub fn new(on_end: Option<EdgyMutation<AnimationEndEvent>>) -> Self {
        ImplicitAnimationHost {
            eased_t: None,
            on_end,
            registered: false,
        }
    }

    /// Lazily register a driver with the manager. Idempotent — the driver is
    /// created once on first call; subsequent calls are no-ops (duration and
    /// curve are treated as fixed per element lifetime; changing them should
    /// remount via `queryKey`). Stores the shared `eased_t` cell for layout
    /// reads.
    pub fn ensure_registered(
        &mut self,
        cx: &SharedViewCx,
        element_id: ElementNodeId,
        duration_ms: u64,
        curve: Curve,
    ) {
        if self.registered {
            return;
        }
        let mgr: Rc<RefCell<AnimationManager>> = cx.js_ctx().animation_manager.clone();
        let eased = mgr
            .borrow_mut()
            .register(element_id, duration_ms, curve, self.on_end.take());
        self.eased_t = Some(eased);
        self.registered = true;
    }

    /// Request a timeline restart from `t = 0`. The element calls this after
    /// rebasing per-prop `begin` values. The actual `start_time` is stamped
    /// on the next frame tick (the manager has the clock; the element's
    /// Effect phase does not).
    pub fn retarget(&self, cx: &SharedViewCx, element_id: ElementNodeId) {
        cx.js_ctx()
            .animation_manager
            .borrow_mut()
            .retarget(element_id);
    }

    /// The latest eased progress reported by the manager. Defaults to `1.0`
    /// (fully settled) before registration completes, so a first paint shows
    /// the target without a flash from `t = 0`.
    pub fn eased_t(&self) -> f64 {
        self.eased_t.as_ref().map(|c| c.get()).unwrap_or(1.0)
    }
}
