//! The shared `LayerLink` handle + the `createLayerLink` factory.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::native_function::NativeFunction;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsResult, JsValue};
use boa_gc::{Finalize, Trace};
use vello_common::kurbo::Affine;

use crate::core::element::ElementNodeId;
use crate::core::js_runtime::{BoaOpaque, JsProps};
use crate::core::layout::Size;

/// Shared, GC-pinned state backing a `LayerLink`. One per
/// `createLayerLink()` call; held by the target element, the follower
/// element, and the [`super::subsystem::CompositedTransformSubsystem`]
/// registry.
///
/// All fields are `Cell`/interior-mutable because the same `Rc` is shared
/// across the build phase (target/follower set their node ids), the subsystem
/// flush (writes the resolved geometry), and paint/hit-test (reads it).
#[derive(Debug)]
pub struct CompositedLinkState {
    /// Set by `CompositedTransformTarget::build`.
    pub target_node: Cell<Option<ElementNodeId>>,
    /// Set by `CompositedTransformFollower::build`.
    pub follower_node: Cell<Option<ElementNodeId>>,

    // --- resolved by the subsystem each flush, read by the follower ---
    /// The target's full world affine (ancestor offsets + paint transforms),
    /// mapping target-local points to canvas space.
    pub target_world: Cell<Affine>,
    /// The target's laid-out size (for resolving `targetAnchor`).
    pub target_size: Cell<Size>,
    /// The follower's tracked **relative transform** (the affine the follower
    /// should apply within its parent's frame), written by the subsystem each
    /// flush. The follower returns this verbatim from `relative_transform`, so
    /// paint, hit-test, and bounds all resolve to the tracked position WITHOUT
    /// storing it in `computed_layout.offset` (which layout owns). Single
    /// ownership — no two writers, so no "flash to top-left" oscillation.
    /// Stored as the full `Affine` (not an `Offset`) so the subsystem can solve
    /// `parent_world⁻¹ · translate(desired)` and correctly track through a
    /// rotated/scaled ancestor `Transform`.
    pub follower_transform: Cell<Affine>,
    /// `true` once at least one flush has resolved a valid target. The
    /// follower uses this to implement `showWhenUnlinked`.
    pub linked: Cell<bool>,
}

impl Default for CompositedLinkState {
    fn default() -> Self {
        Self {
            target_node: Cell::new(None),
            follower_node: Cell::new(None),
            target_world: Cell::new(Affine::IDENTITY),
            target_size: Cell::new(Size::ZERO),
            follower_transform: Cell::new(Affine::IDENTITY),
            linked: Cell::new(false),
        }
    }
}

/// JS-opaque handle wrapping the shared state. Constructed only via
/// `createLayerLink()` (the closure), so the factory can register the state
/// into the subsystem's registry before handing it to JS.
#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct LayerLink(pub Rc<CompositedLinkState>);

impl LayerLink {
    pub fn new(state: Rc<CompositedLinkState>) -> Self {
        Self(state)
    }
}

/// Captures stashed inside the `createLayerLink` JS closure. Held inside
/// boa's GC heap (the closure is wrapped in a `Gc`), so the type must impl
/// `Trace` — but it owns only pure-Rust state (no `Gc`), so the trace is empty.
#[derive(Clone, Trace, Finalize)]
#[boa_gc(unsafe_empty_trace)]
struct LinkRegistryCaptures {
    links: Rc<RefCell<Vec<Rc<CompositedLinkState>>>>,
}

/// Build the `createLayerLink` native closure. Captures the shared link
/// registry (the same `Rc` held by the subsystem) so each newly-created link
/// is tracked for the per-flush recompute.
pub fn build_create_layer_link(links: Rc<RefCell<Vec<Rc<CompositedLinkState>>>>) -> NativeFunction {
    NativeFunction::from_copy_closure_with_captures(
        move |_this, _args, caps, ctx| create_layer_link(&caps.links, ctx),
        LinkRegistryCaptures { links },
    )
}

fn create_layer_link(
    registry: &Rc<RefCell<Vec<Rc<CompositedLinkState>>>>,
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let state = Rc::new(CompositedLinkState::default());
    registry.borrow_mut().push(state.clone());
    let opaque = BoaOpaque::new(LayerLink::new(state), ctx);
    Ok(opaque.object().clone().into())
}

/// Read a `LayerLink`'s shared state off the `link` field of a props object.
/// Returns `None` if absent or not a `LayerLink`.
pub(crate) fn extract_link_state(
    props: &JsObject,
    ctx: &mut Context,
) -> Option<Rc<CompositedLinkState>> {
    let mut p = JsProps::new(props, ctx);
    p.opaque::<LayerLink>("link")
        .and_then(|obj| obj.downcast_ref::<LayerLink>().map(|l| l.0.clone()))
}
