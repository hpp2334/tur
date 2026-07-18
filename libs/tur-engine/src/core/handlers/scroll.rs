use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::elements::{ElementOnWheelContext, NodeTreeData, WheelEvent};
use crate::core::event::{AppEvent, PlatformEvent};
use crate::core::hit_test::HitTest;
use crate::core::layout::Offset;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

use crate::elements::scroll_view::ScrollViewElement;

/// Unified scroll subsystem. Owns the entire scroll event pipeline:
///
/// - **Input** — real device wheel (`PlatformEvent::Wheel`) and derived
///   scroll produced by the gesture arena (`AppEvent::Scroll`). Both feed
///   the same `process_scroll_delta` path: hit-test to the deepest
///   wheel-bearing element, dispatch the delta via `dispatch_wheel`, and
///   forward any residual as `AppEvent::ScrollOverscroll`.
/// - **Chaining** — `AppEvent::ScrollOverscroll` from a child that couldn't
///   consume the full delta is forwarded to the nearest wheel-bearing
///   ancestor (also via `dispatch_wheel`), with any further residual
///   re-emitted as another `AppEvent::ScrollOverscroll`. Cascades across
///   flush iterations via the engine's fixed-point loop.
/// - **Scroll-to** — `AppEvent::ScrollTo { node_id, offset }` (produced by
///   `EditableTextElement.scroll_view_to` and similar APIs) is translated
///   into a delta and routed through `dispatch_wheel`, which clamps, updates
///   controller metrics, fires `onScroll`, requests a paint, and handles
///   chaining.
///
/// The recursive chaining behaviour works because re-emitted events land in
/// `app_event_queue` and are drained on the next iteration of the engine's
/// flush fixed-point loop (see `flush_app_events`).
pub struct ScrollSubsystem;

impl Subsystem for ScrollSubsystem {
    fn handle_platform_event(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        event: &PlatformEvent,
    ) {
        // Real device wheel / trackpad scroll from the platform.
        let PlatformEvent::Wheel {
            delta_x,
            delta_y,
            position,
        } = event
        else {
            return;
        };
        process_scroll_delta(cx, *delta_x, *delta_y, *position);
    }

    fn handle_app_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &AppEvent) {
        match event {
            // Derived scroll produced by the gesture arena (e.g. a touch drag
            // the arena resolved to scroll). Routed through the same pipeline
            // as a real platform wheel so hit-testing, overscroll and chaining
            // behave identically.
            AppEvent::Scroll {
                delta_x,
                delta_y,
                position,
            } => {
                process_scroll_delta(cx, *delta_x, *delta_y, *position);
            }

            // Scroll-chaining: a child couldn't consume the full delta —
            // forward the residual to the nearest wheel-bearing ancestor.
            AppEvent::ScrollOverscroll { source_id, delta } => {
                chain_overscroll(cx, *source_id, *delta);
            }

            // Programmatic scroll-to. Translates the requested absolute
            // offset into a delta and routes it through dispatch_wheel.
            AppEvent::ScrollTo { node_id, offset } => {
                resolve_scroll_to(cx, *node_id, *offset);
            }

            _ => {}
        }
    }
}

// ── Wheel input ───────────────────────────────────────────────────────

/// Shared scroll-delta processing for real (`PlatformEvent::Wheel`) and
/// derived (`AppEvent::Scroll`) scroll: hit-test to the deepest wheel-bearing
/// element, dispatch the delta, and forward any residual as overscroll.
fn process_scroll_delta(
    cx: &mut SubsystemFlushContext<'_>,
    delta_x: f64,
    delta_y: f64,
    position: Offset,
) {
    let (hit_path, target) = {
        let tree = cx.element_tree.borrow();
        let hit_path = HitTest::new(&tree).path(position);
        let target = find_deepest_with_wheel(&tree, &hit_path);
        (hit_path, target)
    };
    let _ = hit_path;

    let Some(target_id) = target else {
        return;
    };

    let overscroll = dispatch_wheel(cx, target_id, delta_x, delta_y);
    if overscroll.abs() > 0.001 {
        cx.app_event_queue.push(AppEvent::ScrollOverscroll {
            source_id: target_id,
            delta: overscroll,
        });
    }
}

fn find_deepest_with_wheel(
    tree: &NodeTreeData,
    hit_path: &[ElementNodeId],
) -> Option<ElementNodeId> {
    for &id in hit_path {
        if let Some(node) = tree.get_element(id)
            && let Some(ref element) = node.element
                && element.has_on_wheel() {
                    return Some(id);
                }
    }
    None
}

// ── Scroll chaining ───────────────────────────────────────────────────

fn chain_overscroll(cx: &mut SubsystemFlushContext<'_>, source_id: ElementNodeId, delta: f64) {
    let parent_id = {
        let tree = cx.element_tree.borrow();
        find_ancestor_with_wheel(&tree, source_id)
    };
    let Some(parent_id) = parent_id else {
        return;
    };

    let overscroll = dispatch_wheel(cx, parent_id, 0.0, delta);
    if overscroll.abs() > 0.001 {
        cx.app_event_queue.push(AppEvent::ScrollOverscroll {
            source_id: parent_id,
            delta: overscroll,
        });
    }
}

/// Walk parents from `start` to find the nearest ancestor with an `onWheel`
/// handler. Hops through fragment ancestors transparently (fragments can't
/// carry wheel handlers, so they're skipped without inspection).
fn find_ancestor_with_wheel(
    tree: &NodeTreeData,
    start: ElementNodeId,
) -> Option<ElementNodeId> {
    let mut current: Option<NodeId> = tree.get_element(start).and_then(|n| n.parent);
    while let Some(id) = current {
        if let Some(node) = tree.get_element(ElementNodeId::new(id.as_u64())) {
            if let Some(ref element) = node.element
                && element.has_on_wheel() {
                    return Some(ElementNodeId::new(id.as_u64()));
                }
            current = node.parent;
        } else if let Some(frag) = tree.get_fragment(FragmentNodeId::new(id.as_u64())) {
            current = Some(frag.parent);
        } else {
            break;
        }
    }
    None
}

// ── Scroll-to ─────────────────────────────────────────────────────────

fn resolve_scroll_to(cx: &mut SubsystemFlushContext<'_>, node_id: ElementNodeId, offset: f64) {
    let current = {
        let tree = cx.element_tree.borrow();
        tree.get_element(node_id)
            .and_then(|n| n.element.as_ref())
            .and_then(|e| e.cast::<ScrollViewElement>())
            .map(|sv| sv.scroll_offset())
            .unwrap_or(0.0)
    };

    let delta = offset - current;
    if delta.abs() > 0.001 {
        dispatch_wheel(cx, node_id, 0.0, delta);
    }
}

// ── Shared wheel dispatch ─────────────────────────────────────────────

/// Dispatch a wheel delta to an element's `on_wheel_event`. Returns the
/// residual overscroll the element reported (positive = couldn't consume).
/// Marks the element dirty for re-layout. Public so cross-crate subsystems
/// (tur-text's `CaretVisibilitySubsystem`) can reuse the same scroll path.
pub fn dispatch_wheel(
    cx: &mut SubsystemFlushContext<'_>,
    id: ElementNodeId,
    delta_x: f64,
    delta_y: f64,
) -> f64 {
    let mut tree = cx.element_tree.borrow_mut();
    let Some(node) = tree.get_element_mut(id) else {
        return 0.0;
    };
    let Some(ref mut element) = node.element else {
        return 0.0;
    };
    let mut mq = cx.mutation_queue.borrow_mut();
    let mut el_cx = ElementOnWheelContext::new(
        &mut *cx.app_event_queue,
        &mut mq,
        cx.need_paint,
        id,
    );
    let overscroll = element.on_wheel_event(&mut el_cx, &WheelEvent { delta_x, delta_y });
    tree.mark_dirty(id.into());
    overscroll
}
