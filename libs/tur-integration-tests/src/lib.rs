use std::cell::Cell;
use std::cell::Ref;
use std::cell::RefCell;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::time::{Duration, Instant};

use tur_engine::core::async_::AsyncRuntime;
use tur_engine::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use tur_engine::core::elements::AnyElement;
use tur_engine::core::elements::NodeTreeData;
use tur_engine::core::event::{AppEvent, AppImeEvent, PlatformEvent, PointerDeviceKind, PointerInput};
use tur_engine::core::fonts::PresetFontLoader;
use tur_engine::core::keyboard::{AppKeyEvent, KeyEventType, Modifiers};
use tur_engine::elements::PointerInteractElement;
use tur_engine::error::TurError;
use tur_engine::renderer::noop::NoopRenderer;
use tur_engine::{TurApp, TurEngine};
use tur_net::{Http, HttpBody, HttpOutcome, RequestOpts, TurNetPlugin};
use tur_std::{Clipboard, CursorPlatform, TurStdPlugin};
use tur_clipboard::TurClipboardPlugin;
use tur_shared::{Cursor, MouseButton, Offset};

/// Wall-clock `AsyncRuntime` for tests. Uses real `Instant::now()` —
/// deterministic timing belongs to boa's `FixedClock` (advanced manually);
/// this is just for wall-clock reads from spawned futures (rare in tests).
#[derive(Default, Clone)]
pub struct TestRuntime;

impl AsyncRuntime for TestRuntime {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// `Clipboard` impl for tests. Reads return a pre-canned value (set via
/// [`Self::set_next_read`]); writes are appended to a log drainable via
/// [`Self::take_writes`] / [`Self::last_write`]. Both resolve eagerly
/// (`std::future::ready`), so the engine's `tick` polls them to completion
/// inside a single `flush` iteration — tests stay deterministic.
#[derive(Default, Clone)]
pub struct RecordingClipboard {
    inner: Rc<RecordingClipboardInner>,
}

#[derive(Default)]
struct RecordingClipboardInner {
    next_read: RefCell<String>,
    writes: RefCell<Vec<String>>,
}

impl RecordingClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-canned text returned by the next `clipboard.read_text().await`.
    pub fn set_next_read(&self, s: impl Into<String>) {
        *self.inner.next_read.borrow_mut() = s.into();
    }

    /// Drain all writes logged so far, in insertion order.
    pub fn take_writes(&self) -> Vec<String> {
        std::mem::take(&mut *self.inner.writes.borrow_mut())
    }

    /// Drain all writes and return the last one (matches the old
    /// `take_clipboard_write` slot semantics).
    pub fn last_write(&self) -> Option<String> {
        self.take_writes().pop()
    }
}

impl Clipboard for RecordingClipboard {
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>> {
        let s = self.inner.next_read.borrow().clone();
        Box::pin(std::future::ready(s))
    }
    fn write_text(&self, text: String) -> Pin<Box<dyn Future<Output = ()>>> {
        self.inner.writes.borrow_mut().push(text);
        Box::pin(std::future::ready(()))
    }
}

/// `Http` impl for tests. Returns a pre-canned [`HttpOutcome`] (set via
/// [`Self::set_next_response`]); logs each incoming [`RequestOpts`] for
/// assertion via [`Self::last_request`]. Resolves eagerly so tests stay
/// deterministic.
#[derive(Default, Clone)]
pub struct RecordingHttp {
    inner: Rc<RecordingHttpInner>,
}

#[derive(Default)]
struct RecordingHttpInner {
    next_response: RefCell<Option<HttpOutcome>>,
    last_request: RefCell<Option<RecordedRequest>>,
}

/// Simplified view of an HTTP request captured by [`RecordingHttp`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub url: String,
    pub method: String,
}

impl RecordingHttp {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-canned response returned by the next `request(opts).await`. If
    /// `None`, the request resolves to `HttpOutcome::Err("no canned response")`.
    pub fn set_next_response(&self, outcome: HttpOutcome) {
        *self.inner.next_response.borrow_mut() = Some(outcome);
    }

    /// The most recent request seen by the recording (or `None` if no
    /// request has been issued).
    pub fn last_request(&self) -> Option<RecordedRequest> {
        self.inner.last_request.borrow().clone()
    }
}

impl Http for RecordingHttp {
    fn request(&self, opts: RequestOpts) -> Pin<Box<dyn Future<Output = HttpOutcome>>> {
        *self.inner.last_request.borrow_mut() = Some(RecordedRequest {
            url: opts.url.clone(),
            method: opts.method.clone(),
        });
        let outcome = self
            .inner
            .next_response
            .borrow()
            .clone()
            .unwrap_or_else(|| HttpOutcome::Err("no canned response".to_string()));
        Box::pin(std::future::ready(outcome))
    }
}

/// Helper to build a canned text response.
pub fn text_response(status: u16, body: impl Into<String>) -> HttpOutcome {
    HttpOutcome::Ok {
        status,
        status_text: "OK".to_string(),
        headers: Vec::new(),
        body: HttpBody::Text(body.into()),
    }
}

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
    /// Shared with the `RecordingCursorPlatform` installed in the engine. The engine
    /// pushes cursor changes here (via `CursorPlatform::set_cursor`); the harness
    /// drains it through `take_current_cursor`.
    cursor_slot: Rc<Cell<Option<Cursor>>>,
    /// Shared with the `RecordingClipboard` installed in the engine. Tests
    /// pre-canned reads via `set_clipboard_read`; assert writes via
    /// `take_clipboard_write`.
    clipboard: RecordingClipboard,
    /// Shared with the `RecordingHttp` installed in the engine (only when
    /// constructed via [`Self::new_with_http`]). `None` for the default
    /// constructor — those tests don't register `builtin:tur/net`.
    http: Option<RecordingHttp>,
    /// Synthetic wall-clock ms used to stamp `PointerInput::PointerDown`
    /// events for engine-side multi-click classification. Advanced in small
    /// steps (well under the 500 ms threshold) on each pointer-down so
    /// consecutive `double_click` / `triple_click` calls register as a
    /// multi-click streak.
    synthetic_time_ms: u64,
}

impl TurTestApp {
    pub fn new(width: f64, height: f64) -> Result<Self, TurError> {
        Self::build(width, height, None)
    }

    /// Construct with `TurNetPlugin` registered against a fresh
    /// [`RecordingHttp`], so tests can drive `request()` from JS. Pre-canned
    /// responses via [`Self::set_http_response`]; capture requests via
    /// [`Self::last_http_request`].
    pub fn new_with_http(width: f64, height: f64) -> Result<Self, TurError> {
        Self::build(width, height, Some(RecordingHttp::new()))
    }

    fn build(
        width: f64,
        height: f64,
        http: Option<RecordingHttp>,
    ) -> Result<Self, TurError> {
        let cursor_slot = Rc::new(Cell::new(None));
        let clipboard = RecordingClipboard::new();
        let mut builder = TurEngine::builder()
            .renderer(Box::new(NoopRenderer::new()))
            .font_loader(Box::new(PresetFontLoader::new()))
            .async_runtime(Rc::new(TestRuntime))
            .plugin(
                TurStdPlugin::builder()
                    .cursor(RecordingCursorPlatform {
                        last: cursor_slot.clone(),
                    })
                    .clipboard(clipboard.clone())
                    .build(),
            )
            .plugin(
                TurClipboardPlugin::builder()
                    .clipboard(clipboard.clone())
                    .build(),
            );
        if let Some(http_impl) = http.clone() {
            builder = builder.plugin(TurNetPlugin::builder().http(http_impl).build());
        }
        let mut inner = builder.build()?;
        inner.push_platform_event(PlatformEvent::Resize {
            logical_width: width as u32,
            logical_height: height as u32,
            dpr: 1.0,
        });
        let _ = inner.spawn_loop_once(Duration::ZERO);
        Ok(Self {
            inner,
            cursor_slot,
            clipboard,
            http,
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
        // Case dist files are ES modules that import `builtin:tur/std` (resolved by
        // the engine's module loader) and call `render(<case default>)`.
        self.inner.load_module(&source)?;
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
        self.inner.push_app_event(AppEvent::RequestDraw);
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    /// Push a viewport resize and flush, exercising the full relayout path.
    pub fn resize(&mut self, width: f64, height: f64) {
        self.inner.push_platform_event(PlatformEvent::Resize {
            logical_width: width as u32,
            logical_height: height as u32,
            dpr: 1.0,
        });
        self.ensure_flushed();
    }

    pub fn tick(&mut self) -> Result<(), TurError> {
        self.inner.spawn_loop_once(Duration::ZERO).map(|_| ())
    }

    pub fn advance(&mut self, duration: Duration) -> Result<(), TurError> {
        self.inner.spawn_loop_once(duration).map(|_| ())
    }

    pub fn element_tree(&self) -> Ref<'_, NodeTreeData> {
        self.inner.element_tree()
    }

    pub fn click(&mut self, x: f64, y: f64) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerDown {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
                device: PointerDeviceKind::Mouse,
            }));
        self.ensure_flushed();
        self.inner
            .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerUp {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                device: PointerDeviceKind::Mouse,
            }));
        self.ensure_flushed();
    }

    pub fn send_key(&mut self, key: &str) {
        self.inner.push_platform_event(PlatformEvent::Key(AppKeyEvent {
            key: key.to_string(),
            code: key.to_string(),
            modifiers: Modifiers::default(),
            event_type: KeyEventType::Down,
        }));
        self.ensure_flushed();
    }

    pub fn send_ime(&mut self, event: AppImeEvent) {
        self.inner.push_platform_event(PlatformEvent::Ime(event));
        self.ensure_flushed();
    }

    pub fn send_key_with_modifiers(&mut self, key: &str, shift: bool, ctrl: bool) {
        self.send_key_with_modifiers_full(key, shift, ctrl, false);
    }

    /// Full-key modifier helper. `meta` covers Cmd on macOS / Win on Windows.
    /// Use this for Cmd+C / Cmd+V / Cmd+S tests.
    pub fn send_key_with_modifiers_full(&mut self, key: &str, shift: bool, ctrl: bool, meta: bool) {
        self.inner.push_platform_event(PlatformEvent::Key(AppKeyEvent {
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
            .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerDown {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
                device: PointerDeviceKind::Mouse,
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
            .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerMove {
                position: Offset::new(x, y),
                device: PointerDeviceKind::Mouse,
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    pub fn pointer_up(&mut self, x: f64, y: f64) {
        self.inner
            .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerUp {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                device: PointerDeviceKind::Mouse,
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    /// Same as `pointer_down` but with an explicit mouse button. Used to
    /// simulate right-click (button 2) without an enclosing `click` gesture.
    pub fn pointer_down_with_button(&mut self, x: f64, y: f64, button: MouseButton) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerDown {
                position: Offset::new(x, y),
                button,
                time_ms,
                device: PointerDeviceKind::Mouse,
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    pub fn pointer_up_with_button(&mut self, x: f64, y: f64, button: MouseButton) {
        self.inner
            .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerUp {
                position: Offset::new(x, y),
                button,
                device: PointerDeviceKind::Mouse,
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    /// Push a right-click sequence: pointer-down(button=Right) then
    /// pointer-up(button=Right). The engine's gesture arena derives the
    /// `ContextMenu` gesture from the right-button pointer-up — there is no
    /// separate context-menu platform event anymore.
    pub fn right_click(&mut self, x: f64, y: f64) {
        self.pointer_down_with_button(x, y, MouseButton::Right);
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
        self.pointer_up_with_button(x, y, MouseButton::Right);
    }

    /// Queue a pointer-down without flushing — used to simulate the browser's
    /// batching of multiple input events between animation frames. Pair with
    /// `pointer_move_no_flush` / `pointer_up_no_flush` and a single `tick()`.
    pub fn pointer_down_no_flush(&mut self, x: f64, y: f64) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerDown {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
                device: PointerDeviceKind::Mouse,
            }));
    }

    pub fn pointer_move_no_flush(&mut self, x: f64, y: f64) {
        self.inner
            .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerMove {
                position: Offset::new(x, y),
                device: PointerDeviceKind::Mouse,
            }));
    }

    pub fn pointer_up_no_flush(&mut self, x: f64, y: f64) {
        self.inner
            .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerUp {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                device: PointerDeviceKind::Mouse,
            }));
    }

    pub fn wheel(&mut self, delta_x: f64, delta_y: f64, x: f64, y: f64) {
        self.inner
            .push_platform_event(PlatformEvent::Wheel {
                delta_x,
                delta_y,
                position: Offset::new(x, y),
            });
        self.ensure_flushed();
    }

    /// Drive `spawn_loop_once` for a few iterations to settle cascading
    /// reactive updates, async completions, and PromiseJobs. Public so
    /// external tests (e.g. async bridge tests) can use the same pattern.
    pub fn ensure_flushed(&mut self) {
        for _ in 0..6 {
            let _ = self.inner.spawn_loop_once(Duration::from_millis(3));
        }
    }

    pub fn has_click_handler(&self, id: ElementNodeId) -> bool {
        self.inner.with_element(id, |e| {
            e.cast::<PointerInteractElement>()
                .map(|p| p.has_on_click())
                .unwrap_or(false)
        }).unwrap_or(false)
    }

    pub fn has_mouse_region_callbacks(&self, id: ElementNodeId) -> bool {
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

    pub fn get_element_absolute_bounds(&self, id: ElementNodeId) -> Option<Rect> {
        let tree = self.inner.element_tree();
        let node = tree.get_element(id)?;
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

    pub fn focused_element(&self) -> Option<ElementNodeId> {
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
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        self.inner.with_element(id, cb)
    }

    /// Returns the most recent cursor pushed by the engine since the last
    /// call. The engine pushes cursor changes through the `RecordingCursorPlatform`
    /// during `apply_changes`; this drains that recording.
    pub fn take_current_cursor(&self) -> Option<Cursor> {
        self.cursor_slot.take()
    }

    /// Drain any text written to the clipboard via `AppEvent::ClipboardWrite`
    /// (e.g. EditableText's Cmd+C / Cmd+X handling) since the last call.
    /// Returns the latest write (the `RecordingClipboard` logs every write;
    /// this drains all and returns the last, matching the old slot semantics).
    pub fn take_clipboard_write(&self) -> Option<String> {
        self.clipboard.last_write()
    }

    /// Pre-canned text returned by the next `clipboardReadText()` call from
    /// JS, or `set_source` on a reactive atom driven by it. Useful for
    /// testing paste-via-read flows.
    pub fn set_clipboard_read(&self, s: impl Into<String>) {
        self.clipboard.set_next_read(s);
    }

    /// Access the raw recording for advanced assertions (e.g. asserting
    /// multiple writes happen in order).
    pub fn clipboard(&self) -> &RecordingClipboard {
        &self.clipboard
    }

    /// Pre-canned response for the next `request(opts).await` from JS.
    /// Panics if this app wasn't constructed via [`Self::new_with_http`].
    pub fn set_http_response(&self, outcome: HttpOutcome) {
        self.http
            .as_ref()
            .expect("TurTestApp::set_http_response requires new_with_http")
            .set_next_response(outcome);
    }

    /// The most recent request seen by the recording, or `None` if no
    /// request has been issued. Panics if not constructed with http.
    pub fn last_http_request(&self) -> Option<RecordedRequest> {
        self.http
            .as_ref()
            .expect("TurTestApp::last_http_request requires new_with_http")
            .last_request()
    }

    /// Push a synthetic paste event — equivalent to the embedder firing
    /// `paste` on the hidden textarea. The engine's `ClipboardPasteHandler`
    /// then inserts `text` into the focused editable.
    pub fn push_paste_event(&mut self, text: &str) {
        self.inner
            .push_platform_event(PlatformEvent::ClipboardPaste { text: text.to_string() });
        self.ensure_flushed();
    }

    pub fn eval_js(&mut self, source: &str) -> String {
        self.inner.eval_js(source).unwrap_or_default()
    }

    pub fn load_bundle_source(&mut self, source: &str) -> Result<(), TurError> {
        self.inner.load_js(source)
    }

    /// Evaluate `source` as an ES module — supports real
    /// `import { … } from "builtin:tur/std"` (or `builtin:tur/host`/`builtin:tur/net`). Returns
    /// nothing; read results back via [`eval_js`](Self::eval_js).
    pub fn eval_module_source(&mut self, source: &str) -> Result<(), TurError> {
        self.inner.load_module(source)
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

/// Test `CursorPlatform` that records the last cursor the engine pushed. Shares its
/// slot (via `Rc<Cell>`) with [`TurTestApp`], which drains it through
/// `take_current_cursor`.
#[derive(Clone)]
struct RecordingCursorPlatform {
    last: Rc<Cell<Option<Cursor>>>,
}

impl CursorPlatform for RecordingCursorPlatform {
    fn set_cursor(&mut self, cursor: Cursor) {
        self.last.set(Some(cursor));
    }
}
