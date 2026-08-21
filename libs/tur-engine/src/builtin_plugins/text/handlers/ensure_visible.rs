use crate::builtin_plugins::clipboard::ClipboardPasteEvent;
use crate::builtin_plugins::scroll::ScrollViewElement;
use crate::builtin_plugins::scroll::dispatch_wheel;
use crate::core::app::AppEvent;
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::elements::NodeTreeData;
use crate::core::layout::Axis;
use crate::core::platform::PlatformEvent;
use crate::core::shell::ShellEvent;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

use crate::builtin_plugins::text::elements::editable_text::EditableTextElement;

/// Post-subsystem that keeps the caret on screen after text-moving events
/// (keyboard, IME, clipboard-paste). Registered by [`crate::install_text_feature`]
/// after the engine's `KeyboardSubsystem` / `ImeSubsystem` (for keyboard /
/// IME caret moves) and after tur-text's `ClipboardPasteSubsystem` (for paste
/// caret moves), so by the time this subsystem runs the focused editable's
/// buffer + caret have already been updated.
///
/// Subscribes to two event streams:
/// - `PlatformEvent::Key` / `PlatformEvent::Ime` — keyboard / IME caret moves
///   happen synchronously in the engine's `KeyboardSubsystem` /
///   `ImeSubsystem`, so the post-subsystem can observe them in the same
///   platform-event pass.
/// - [`ClipboardPasteEvent`] (inside `AppEvent::Custom`) — paste is forwarded
///   from `ClipboardPlatformPasteEvent` (platform) to
///   [`ClipboardPasteEvent`] (app) by tur-clipboard's
///   `ClipboardPlatformSubsystem`, then consumed by tur-text's
///   `ClipboardPasteSubsystem`. Since AppEvents drain in a later flush pass
///   than the originating PlatformEvent (the queues are snapshotted at the
///   start of `flush_app_events`), this subsystem must subscribe to the
///   AppEvent — subscribing to the platform paste would run before the paste
///   happens.
///
/// No-op when the focused element isn't a multiline `EditableText`, no
/// scrollable ancestor exists, or the caret line is already visible.
pub struct CaretVisibilitySubsystem;

impl Subsystem for CaretVisibilitySubsystem {
    fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent) {
        // Only caret-moving events warrant a scroll. Resize / pointer / wheel
        // events don't move the caret. (Paste is handled in
        // `handle_app_event` — see the struct doc for why.)
        match event {
            PlatformEvent::Shell(ShellEvent::Key(_)) | PlatformEvent::Shell(ShellEvent::Ime(_)) => {
                ensure_caret_visible(cx);
            }
            _ => {}
        }
    }

    fn handle_app_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &AppEvent) {
        if event.as_custom::<ClipboardPasteEvent>().is_some() {
            ensure_caret_visible(cx);
        }
    }
}

/// If the focused element is a multiline `EditableText` living inside a
/// `ScrollView`, scroll the nearest scrollable ancestor just enough to bring
/// the caret line into view. No-op otherwise (no focus, not an editable, no
/// scroll ancestor, or the caret is already visible).
///
/// Reads the editable's cached text layout from the previous frame; for pure
/// cursor moves this is exact, for typed text the correction lags one frame
/// and self-corrects.
pub fn ensure_caret_visible(cx: &mut SubsystemFlushContext<'_>) {
    let Some(focused) = cx.focus_manager.borrow().focused() else {
        return;
    };

    let metrics = {
        let tree = cx.element_tree.borrow();
        let Some((line_top, line_height)) = caret_line_geom(&tree, focused) else {
            return;
        };
        let Some(scroll_id) = nearest_scroll_ancestor(&tree, focused) else {
            return;
        };
        // Absolute Y of the caret and of the scroll viewport, obtained by
        // accumulating each node's offset relative to its parent up to the root.
        let caret_abs_top = abs_offset_y(&tree, focused) + line_top as f64;
        let scroll_abs_top = abs_offset_y(&tree, scroll_id);

        let Some((axis, current, viewport_main, max_extent)) = scroll_metrics(&tree, scroll_id)
        else {
            return;
        };
        (
            axis,
            current,
            viewport_main,
            max_extent,
            scroll_id,
            caret_abs_top,
            scroll_abs_top,
            line_height,
        )
    };
    let (
        axis,
        current,
        viewport_main,
        max_extent,
        scroll_id,
        caret_abs_top,
        scroll_abs_top,
        line_height,
    ) = metrics;

    if axis != Axis::Vertical {
        return;
    }

    // Content-space caret Y is invariant under the scroll offset (the offset
    // cancels against the viewport translation), so a single measurement
    // suffices regardless of the current scroll position.
    let content_top = caret_abs_top - scroll_abs_top + current;
    let content_bottom = content_top + line_height as f64;

    let new_offset = if content_top < current {
        content_top
    } else if content_bottom > current + viewport_main {
        content_bottom - viewport_main
    } else {
        return; // already visible
    };

    let clamped = new_offset.clamp(0.0, max_extent.max(0.0));
    let delta = clamped - current;
    if delta.abs() < 0.5 {
        return;
    }

    // Reuse the wheel path: it clamps, updates controller metrics, fires the
    // onScroll callback, requests a paint, and handles scroll chaining. The
    // delta is pre-clamped, so no spurious overscroll is produced.
    dispatch_wheel(cx, scroll_id, 0.0, delta);
}

/// `(line_top, line_height)` for the focused caret, in the editable's local
/// coords. `None` if `id` isn't an `EditableTextElement` or has no layout yet.
fn caret_line_geom(tree: &NodeTreeData, id: ElementNodeId) -> Option<(f32, f32)> {
    let node = tree.get_element(id)?;
    let element = node.element.as_ref()?;
    let editable = element.cast::<EditableTextElement>()?;
    let layout = editable.cached_layout.as_ref()?;
    let line = layout.line_index_for_byte(editable.cursor_position());
    let info = layout.line_infos.get(line)?;
    Some((info.top, info.height))
}

/// Walk parents from `start` to find the nearest `ScrollViewElement`. Hops
/// through fragment ancestors transparently (fragments have no element, so
/// without the hop the walk would silently terminate at the first fragment).
fn nearest_scroll_ancestor(tree: &NodeTreeData, start: ElementNodeId) -> Option<ElementNodeId> {
    let mut current: Option<NodeId> = tree.get_element(start).and_then(|n| n.parent);
    while let Some(id) = current {
        if let Some(node) = tree.get_element(ElementNodeId::new(id.as_u64())) {
            if let Some(ref element) = node.element
                && element.cast::<ScrollViewElement>().is_some()
            {
                return Some(ElementNodeId::new(id.as_u64()));
            }
            current = node.parent;
        } else if let Some(frag) = tree.get_fragment(FragmentNodeId::new(id.as_u64())) {
            // Fragments can't be ScrollView; hop to the next ancestor.
            current = Some(frag.parent);
        } else {
            break;
        }
    }
    None
}

/// Sum of `computed_layout.offset.y` from `start` up to the root (inclusive of
/// `start`'s own offset within its parent). Hops through fragment ancestors
/// transparently (fragments have zero offset).
fn abs_offset_y(tree: &NodeTreeData, start: ElementNodeId) -> f64 {
    let mut acc = 0.0f64;
    let mut current: Option<NodeId> = Some(start.into());
    while let Some(id) = current {
        if let Some(n) = tree.get_element(ElementNodeId::new(id.as_u64())) {
            acc += n.computed_layout.offset.y;
            current = n.parent;
        } else if let Some(frag) = tree.get_fragment(FragmentNodeId::new(id.as_u64())) {
            current = Some(frag.parent);
        } else {
            break;
        }
    }
    acc
}

/// `(axis, current_offset, viewport_main_extent, max_scroll_extent)`.
fn scroll_metrics(tree: &NodeTreeData, id: ElementNodeId) -> Option<(Axis, f64, f64, f64)> {
    let node = tree.get_element(id)?;
    let element = node.element.as_ref()?;
    let scroll = element.cast::<ScrollViewElement>()?;
    let viewport = scroll.viewport_size();
    let axis = scroll.axis();
    let viewport_main = match axis {
        Axis::Vertical => viewport.height,
        Axis::Horizontal => viewport.width,
    };
    Some((
        axis,
        scroll.scroll_offset(),
        viewport_main,
        scroll.max_scroll_extent(),
    ))
}
