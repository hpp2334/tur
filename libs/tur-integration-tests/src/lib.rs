use std::cell::Cell;
use std::cell::Ref;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use tur_engine::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use tur_engine::core::elements::AnyElement;
use tur_engine::core::elements::ElementTree;
use tur_engine::core::event::{AppEvent, AppGestureEvent, AppImeEvent};
use tur_engine::core::fonts::PresetFontLoader;
use tur_engine::core::keyboard::{AppKeyEvent, KeyEventType, Modifiers};
use tur_engine::elements::PointerInteractElement;
use tur_engine::error::TurError;
use tur_engine::renderer::noop::NoopRenderer;
use tur_engine::TurApp;
use tur_shared::{Cursor, MouseButton, Offset};

pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    pub fn center(&self) -> (f64, f64) {
        ((self.left + self.right) / 2.0, (self.top + self.bottom) / 2.0)
    }
}

pub struct TurTestApp {
    inner: TurApp,
    /// Shared with the `RecordingHostApi` installed in the engine. The engine
    /// pushes cursor changes here (via `HostApi::set_cursor`); the harness
    /// drains it through `take_current_cursor`.
    cursor_slot: Rc<Cell<Option<Cursor>>>,
    /// Synthetic wall-clock ms used to stamp `AppGestureEvent::PointerDown`
    /// events for engine-side multi-click classification. Advanced in small
    /// steps (well under the 500 ms threshold) on each pointer-down so
    /// consecutive `double_click` / `triple_click` calls register as a
    /// multi-click streak.
    synthetic_time_ms: u64,
}

impl TurTestApp {
    pub fn new(width: f64, height: f64) -> Result<Self, TurError> {
        let cursor_slot = Rc::new(Cell::new(None));
        let mut inner = TurApp::new(
            Box::new(NoopRenderer::new()),
            Box::new(PresetFontLoader::new()),
            Box::new(RecordingHostApi {
                last: cursor_slot.clone(),
            }),
        )?;
        inner.push_event(AppEvent::Resize {
            logical_width: width as u32,
            logical_height: height as u32,
            dpr: 1.0,
        });
        let _ = inner.spawn_loop_once(Duration::ZERO);
        Ok(Self {
            inner,
            cursor_slot,
            synthetic_time_ms: 1_700_000_000_000, // arbitrary stable epoch base
        })
    }

    /// Bump the synthetic time source so the next pointer-down stamps a
    /// fresh `time_ms`. Default step is small enough to stay inside the
    /// engine's 500 ms multi-click window.
    fn bump_time(&mut self, step_ms: u64) -> u64 {
        self.synthetic_time_ms = self.synthetic_time_ms.saturating_add(step_ms);
        self.synthetic_time_ms
    }

    /// Test-only hook to advance the synthetic wall-clock without sending
    /// any event. Useful for pushing past the engine's multi-click
    /// classification window (e.g. to simulate a single click that
    /// follows a double-click after a long pause).
    pub fn bump_synthetic_time_ms_for_test(&mut self, step_ms: u64) {
        let _ = self.bump_time(step_ms);
    }

    pub fn load_bundle(&mut self, name: &str) -> Result<(), TurError> {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let workspace_root = Path::new(&manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .expect("failed to resolve workspace root");
        let path = workspace_root
            .join("js/packages/tur-test-cases/dist")
            .join(format!("{name}.js"));
        let source = std::fs::read_to_string(&path).map_err(TurError::Io)?;
        self.inner.load_js(&source)?;
        self.ensure_flushed();
        Ok(())
    }

    /// Direct mutable access to the underlying `TurApp` — lets a test register
    /// extra `__tur.*` / `__turHost.*` fns (e.g. a fake `__tur.request` backed
    /// by an in-process WebDAV server) before loading a bundle.
    pub fn with_app_mut<R>(&mut self, f: impl FnOnce(&mut TurApp) -> R) -> R {
        f(&mut self.inner)
    }

    pub fn render(&mut self) {
        self.inner.push_event(AppEvent::RequestDraw);
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    /// Push a viewport resize and flush, exercising the full relayout path.
    pub fn resize(&mut self, width: f64, height: f64) {
        self.inner.push_event(AppEvent::Resize {
            logical_width: width as u32,
            logical_height: height as u32,
            dpr: 1.0,
        });
        self.ensure_flushed();
    }

    pub fn tick(&mut self) -> Result<(), TurError> {
        self.inner.spawn_loop_once(Duration::ZERO)
    }

    pub fn advance(&mut self, duration: Duration) -> Result<(), TurError> {
        self.inner.spawn_loop_once(duration)
    }

    pub fn element_tree(&self) -> Ref<'_, ElementTree> {
        self.inner.element_tree()
    }

    pub fn click(&mut self, x: f64, y: f64) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerDown {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
            }));
        self.ensure_flushed();
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerUp {
                position: Offset::new(x, y),
                button: MouseButton::Left,
            }));
        self.ensure_flushed();
    }

    pub fn send_key(&mut self, key: &str) {
        self.inner.push_event(AppEvent::Key(AppKeyEvent {
            key: key.to_string(),
            code: key.to_string(),
            modifiers: Modifiers::default(),
            event_type: KeyEventType::Down,
        }));
        self.ensure_flushed();
    }

    pub fn send_ime(&mut self, event: AppImeEvent) {
        self.inner.push_event(AppEvent::Ime(event));
        self.ensure_flushed();
    }

    pub fn send_key_with_modifiers(&mut self, key: &str, shift: bool, ctrl: bool) {
        self.send_key_with_modifiers_full(key, shift, ctrl, false);
    }

    /// Full-key modifier helper. `meta` covers Cmd on macOS / Win on Windows.
    /// Use this for Cmd+C / Cmd+V / Cmd+S tests.
    pub fn send_key_with_modifiers_full(&mut self, key: &str, shift: bool, ctrl: bool, meta: bool) {
        self.inner.push_event(AppEvent::Key(AppKeyEvent {
            key: key.to_string(),
            code: key.to_string(),
            modifiers: Modifiers {
                shift,
                ctrl,
                meta,
                ..Default::default()
            },
            event_type: KeyEventType::Down,
        }));
        self.ensure_flushed();
    }

    pub fn pointer_down(&mut self, x: f64, y: f64) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerDown {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    /// Simulate a double-click at `(x, y)`. Two `pointer_down`s are pushed in
    /// quick succession (40 ms apart, well inside the engine's 500 ms window)
    /// at the same position, so the gesture composer classifies the second
    /// one as `PointerDoubleDown`.
    pub fn double_click(&mut self, x: f64, y: f64) {
        self.pointer_down(x, y);
        self.pointer_down(x, y);
    }

    /// Simulate a triple-click at `(x, y)`. Three `pointer_down`s in quick
    /// succession — the third one is classified as `PointerTripleDown`.
    pub fn triple_click(&mut self, x: f64, y: f64) {
        self.pointer_down(x, y);
        self.pointer_down(x, y);
        self.pointer_down(x, y);
    }

    pub fn pointer_move(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerMove {
                position: Offset::new(x, y),
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    pub fn pointer_up(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerUp {
                position: Offset::new(x, y),
                button: MouseButton::Left,
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    /// Same as `pointer_down` but with an explicit mouse button. Used to
    /// simulate right-click (button 2) without an enclosing `click` gesture.
    pub fn pointer_down_with_button(&mut self, x: f64, y: f64, button: MouseButton) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerDown {
                position: Offset::new(x, y),
                button,
                time_ms,
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    pub fn pointer_up_with_button(&mut self, x: f64, y: f64, button: MouseButton) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerUp {
                position: Offset::new(x, y),
                button,
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    /// Push a right-click sequence: pointer-down(button=Right), context-menu,
    /// pointer-up(button=Right). Mirrors the DOM event order.
    pub fn right_click(&mut self, x: f64, y: f64) {
        self.pointer_down_with_button(x, y, MouseButton::Right);
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::ContextMenu {
                position: Offset::new(x, y),
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
        self.pointer_up_with_button(x, y, MouseButton::Right);
    }

    /// Queue a pointer-down without flushing — used to simulate the browser's
    /// batching of multiple input events between animation frames. Pair with
    /// `pointer_move_no_flush` / `pointer_up_no_flush` and a single `tick()`.
    pub fn pointer_down_no_flush(&mut self, x: f64, y: f64) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerDown {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
            }));
    }

    pub fn pointer_move_no_flush(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerMove {
                position: Offset::new(x, y),
            }));
    }

    pub fn pointer_up_no_flush(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerUp {
                position: Offset::new(x, y),
                button: MouseButton::Left,
            }));
    }

    pub fn wheel(&mut self, delta_x: f64, delta_y: f64, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Wheel {
                delta_x,
                delta_y,
                position: Offset::new(x, y),
            });
        self.ensure_flushed();
    }

    fn ensure_flushed(&mut self) {
        for _ in 0..6 {
            let _ = self.inner.spawn_loop_once(Duration::from_millis(3));
        }
    }

    pub fn has_click_handler(&self, id: NodeId) -> bool {
        self.inner.with_element(id, |e| {
            e.cast::<PointerInteractElement>()
                .map(|p| p.has_on_click())
                .unwrap_or(false)
        }).unwrap_or(false)
    }

    pub fn has_mouse_region_callbacks(&self, id: NodeId) -> bool {
        use tur_engine::elements::MouseRegionElement;
        self.inner.with_element(id, |e| {
            e.cast::<MouseRegionElement>()
                .map(|m| m.has_region_callbacks())
                .unwrap_or(false)
        }).unwrap_or(false)
    }

    pub fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        self.inner.query_element(key)
    }

    pub fn get_element_absolute_bounds(&self, id: NodeId) -> Option<Rect> {
        let tree = self.inner.element_tree();
        let node = tree.get_element(ElementNodeId::new(id.as_u64()))?;
        let mut x = node.computed_layout.offset.x;
        let mut y = node.computed_layout.offset.y;
        let mut current = node.parent;
        while let Some(cid) = current {
            if let Some(n) = tree.get_element(ElementNodeId::new(cid.as_u64())) {
                x += n.computed_layout.offset.x;
                y += n.computed_layout.offset.y;
                current = n.parent;
            } else if let Some(f) = tree.get_fragment(FragmentNodeId::new(cid.as_u64())) {
                // Fragments have zero offset; hop to their real-ancestor parent.
                current = Some(f.parent);
            } else {
                break;
            }
        }
        Some(Rect {
            left: x,
            top: y,
            right: x + node.computed_layout.size.width,
            bottom: y + node.computed_layout.size.height,
        })
    }

    pub fn focused_element(&self) -> Option<NodeId> {
        self.inner.focused_element()
    }

    pub fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        self.inner.focused_cursor_rect()
    }

    pub fn focused_is_editable(&self) -> bool {
        self.inner.focused_is_editable()
    }

    pub fn with_element<R>(
        &self,
        id: NodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        self.inner.with_element(id, cb)
    }

    /// Returns the most recent cursor pushed by the engine since the last
    /// call. The engine pushes cursor changes through the `RecordingHostApi`
    /// during `apply_changes`; this drains that recording.
    pub fn take_current_cursor(&self) -> Option<Cursor> {
        self.cursor_slot.take()
    }

    /// Drain any text written to the clipboard via `AppEvent::ClipboardWrite`
    /// (e.g. EditableText's Cmd+C / Cmd+X handling) since the last call.
    /// Mirrors the embedder's per-frame clipboard-write poll.
    pub fn take_clipboard_write(&self) -> Option<String> {
        self.inner.take_clipboard_write()
    }

    /// Push a synthetic paste event — equivalent to the embedder firing
    /// `paste` on the hidden textarea. The engine's `ClipboardPasteHandler`
    /// then inserts `text` into the focused editable.
    pub fn push_paste_event(&mut self, text: &str) {
        self.inner
            .push_event(AppEvent::ClipboardPaste { text: text.to_string() });
        self.ensure_flushed();
    }

    pub fn eval_js(&mut self, source: &str) -> String {
        self.inner.eval_js(source).unwrap_or_default()
    }

    pub fn load_bundle_source(&mut self, source: &str) -> Result<(), TurError> {
        self.inner.load_js(source)
    }

    /// Structured dev-tool snapshot of the root node, or `None` if no tree
    /// is mounted. Children are bare ids; iterate with `dev_tool_get_element`.
    pub fn dev_tool_element_tree(&self) -> Option<tur_engine::core::elements::DevNodeData> {
        self.inner.dev_tool_element_tree()
    }

    /// Structured dev-tool snapshot of an arbitrary node by id.
    pub fn dev_tool_get_element(
        &self,
        id: NodeId,
    ) -> Option<tur_engine::core::elements::DevNodeData> {
        self.inner.dev_tool_get_element(id)
    }
}

/// Test `HostApi` that records the last cursor the engine pushed. Shares its
/// slot (via `Rc<Cell>`) with [`TurTestApp`], which drains it through
/// `take_current_cursor`.
struct RecordingHostApi {
    last: Rc<Cell<Option<Cursor>>>,
}

impl tur_engine::core::host_api::HostApi for RecordingHostApi {
    fn set_cursor(&mut self, cursor: Cursor) {
        self.last.set(Some(cursor));
    }
}
