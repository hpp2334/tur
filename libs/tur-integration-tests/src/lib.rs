pub mod test_scheduler;
pub use test_scheduler::{TestHostLoop, TestSchedulerDriver, TestVsyncSource};

use std::cell::RefCell;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use boa_engine::Context;
use boa_engine::NativeFunction;
use boa_engine::context::time::Clock;
use futures::StreamExt;
use futures::executor::block_on;
use tur_engine::TurStdPlugin;
use tur_engine::core::app::{FrameOutcome, NextFrame};
use tur_engine::core::element::{ElementNodeId, NodeId};

/// `Send + Sync` wrapper around boa's `FixedClock` (which uses `RefCell`
/// internally and is therefore `!Sync`). The runtime requires
/// `Arc<dyn Clock + Send + Sync>` so its config can be shared across
/// worker threads (Phase 8 threaded mode). Tests are single-threaded but
/// must satisfy the same bound — `Mutex` adds negligible overhead for the
/// test's per-frame clock access.
pub struct MutexFixedClock(pub std::sync::Mutex<boa_engine::context::time::FixedClock>);

impl boa_engine::context::time::Clock for MutexFixedClock {
    fn now(&self) -> boa_engine::context::time::JsInstant {
        self.0.lock().unwrap().now()
    }
    fn system_time_millis(&self) -> i64 {
        self.0.lock().unwrap().system_time_millis()
    }
}

impl MutexFixedClock {
    pub fn new(start_millis: u64) -> Self {
        Self(std::sync::Mutex::new(
            boa_engine::context::time::FixedClock::from_millis(start_millis),
        ))
    }
    pub fn forward(&self, millis: u64) {
        self.0.lock().unwrap().forward(millis);
    }
}
use tur_engine::core::elements::AnyElement;
use tur_engine::core::elements::{NodeTreeData, NodeTreeSnapshot};
use tur_engine::core::layout::{MouseButton, Offset};
use tur_engine::core::platform::key_event::{KeyEvent, KeyEventType, Modifiers};
use tur_engine::core::platform::{ImeEvent, PointerDeviceKind, PointerInput};
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::core::render::Renderer;
use tur_engine::core::scheduler::WorkerPoolHandle;
use tur_engine::core::shell::{Cursor, ShellEvent, TextInputState};
use tur_engine::error::TurError;
use tur_engine::renderer::noop::NoopRenderer;
use tur_engine::{Clipboard, ClipboardBackend, TurClipboardPlugin};
use tur_engine::{TurApp, TurRuntime};
use tur_filepicker_capability::{
    FilePicker, FilePickerBackend, PickOptions, PickedFile, SaveOptions, TurFilePickerPlugin,
};
use tur_native::NativeFontLoader;
use tur_net_capability::{
    Http, HttpBackend, HttpBody, HttpFuture, HttpOutcome, HttpStreamFuture, HttpStreamResponse,
    RequestOpts, TurNetPlugin,
};

/// A minimal [`Plugin`] that registers a single ctx-free host module at
/// build time. Test-only convenience for the cases that previously used the
/// runtime `TurApp::register_native_module` API (now removed) — lets a test
/// inject `tur:<whatever>` exports through the plugin path.
///
/// **Phase 7**: holds builder closures (not pre-built `NativeFunction`s)
/// because `NativeFunction` wraps a boa `TraceableClosure` (`!Send`). Each
/// instance's `register()` calls the builder to produce a fresh
/// `NativeFunction` against its own boa `Context`.
pub struct NativeModulePlugin {
    /// Module specifier to register (e.g. `"tur:test"`).
    pub specifier: &'static str,
    /// `(name, builder, length)` exports.
    pub exports: Vec<NativeExport>,
}

/// One export of a [`NativeModulePlugin`]. The `builder` closure produces a
/// fresh `NativeFunction` for each instance (called inside `register`).
pub struct NativeExport {
    pub name: String,
    pub builder: Box<dyn Fn(&mut Context) -> NativeFunction + Send + Sync>,
    pub length: usize,
}

impl Plugin for NativeModulePlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let exports: Vec<(String, NativeFunction, usize)> = self
            .exports
            .iter()
            .map(|e| (e.name.clone(), (e.builder)(ctx.boa_mut()), e.length))
            .collect();
        ctx.register_native_module(self.specifier, exports);
        Ok(())
    }
}

/// Fixed per-frame time step (ms) used by [`TurTestApp::wait_frames`] and
/// [`TurTestApp::wait_for`] — 60 fps. Animation/timer tests express elapsed
/// time as a frame count rather than a wall duration.
const FRAME_STEP_MS: u64 = 16;

/// Legacy fixture adapter: if `source` doesn't already contain an `export`
/// (inline fixtures never do; dist bundles and hand-written contract
/// modules always do), hoist its import statements to the top and wrap the
/// remaining statements in `export function start({ store }) { … }` so the
/// source satisfies the module lifecycle contract without hand-editing
/// every inline test bundle. The injected `{ store }` is the instance
/// store; the fixture body's bare `store` references resolve to it.
fn wrap_legacy_start(source: &str) -> String {
    if source.contains("export") {
        return source.to_string();
    }
    let mut imports = String::new();
    let mut body = String::new();
    let mut in_import = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if in_import || trimmed.starts_with("import ") {
            in_import = !trimmed.contains(" from ") && !trimmed.ends_with(';');
            imports.push_str(line);
            imports.push('\n');
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    format!("{imports}export function start({{ store }}) {{\n{body}}}\n")
}

/// `Clipboard` impl for tests. Reads return a pre-canned value (set via
/// [`Self::set_next_read`]); writes are appended to a log drainable via
/// [`Self::take_writes`] / [`Self::last_write`]. Both resolve eagerly
/// (`std::future::ready`), so the engine's `tick` polls them to completion
/// inside a single `flush` iteration — tests stay deterministic.
#[derive(Default, Clone)]
pub struct RecordingClipboard {
    inner: std::sync::Arc<RecordingClipboardInner>,
}

#[derive(Default)]
struct RecordingClipboardInner {
    next_read: std::sync::Mutex<String>,
    writes: std::sync::Mutex<Vec<String>>,
}

impl RecordingClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-canned text returned by the next `clipboard.read_text().await`.
    pub fn set_next_read(&self, s: impl Into<String>) {
        *self.inner.next_read.lock().unwrap() = s.into();
    }

    /// Drain all writes logged so far, in insertion order.
    pub fn take_writes(&self) -> Vec<String> {
        std::mem::take(&mut *self.inner.writes.lock().unwrap())
    }

    /// Drain all writes and return the last one (matches the old
    /// `take_clipboard_write` slot semantics).
    pub fn last_write(&self) -> Option<String> {
        self.take_writes().pop()
    }
}

impl ClipboardBackend for RecordingClipboard {
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>> {
        let s = self.inner.next_read.lock().unwrap().clone();
        Box::pin(std::future::ready(s))
    }
    fn write_text(&self, text: String) -> Pin<Box<dyn Future<Output = ()>>> {
        self.inner.writes.lock().unwrap().push(text);
        Box::pin(std::future::ready(()))
    }
}

/// `Http` impl for tests. Returns a pre-canned [`HttpOutcome`] (set via
/// [`Self::set_next_response`]); logs each incoming [`RequestOpts`] for
/// assertion via [`Self::last_request`]. Resolves eagerly so tests stay
/// deterministic.
#[derive(Default, Clone)]
pub struct RecordingHttp {
    inner: std::sync::Arc<RecordingHttpInner>,
}

#[derive(Default)]
struct RecordingHttpInner {
    next_response: std::sync::Mutex<Option<HttpOutcome>>,
    next_stream_chunks: std::sync::Mutex<Option<(u16, Vec<Vec<u8>>)>>,
    last_request: std::sync::Mutex<Option<RecordedRequest>>,
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
        *self.inner.next_response.lock().unwrap() = Some(outcome);
    }

    /// Pre-canned streaming response: returns the given status + chunks via
    /// `request_stream`. The next `request_stream` call drains these.
    pub fn set_next_stream(&self, status: u16, chunks: Vec<Vec<u8>>) {
        *self.inner.next_stream_chunks.lock().unwrap() = Some((status, chunks));
    }

    /// The most recent request seen by the recording (or `None` if no
    /// request has been issued).
    pub fn last_request(&self) -> Option<RecordedRequest> {
        self.inner.last_request.lock().unwrap().clone()
    }
}

impl HttpBackend for RecordingHttp {
    fn request(&self, opts: RequestOpts) -> HttpFuture {
        *self.inner.last_request.lock().unwrap() = Some(RecordedRequest {
            url: opts.url.clone(),
            method: opts.method.clone(),
        });
        let outcome = self
            .inner
            .next_response
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| HttpOutcome::Err("no canned response".to_string()));
        Box::pin(std::future::ready(outcome))
    }

    fn request_stream(&self, opts: RequestOpts) -> HttpStreamFuture {
        use futures::stream;
        *self.inner.last_request.lock().unwrap() = Some(RecordedRequest {
            url: opts.url.clone(),
            method: opts.method.clone(),
        });
        let canned = self.inner.next_stream_chunks.lock().unwrap().clone();
        match canned {
            Some((status, chunks)) => {
                let body_stream = stream::iter(chunks.into_iter().map(Ok)).boxed_local();
                Box::pin(std::future::ready(Ok(HttpStreamResponse {
                    status,
                    status_text: "OK".to_string(),
                    headers: Vec::new(),
                    body: body_stream,
                })))
            }
            None => Box::pin(std::future::ready(Err("no canned stream".to_string()))),
        }
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

/// `FilePicker` impl for tests. Returns a pre-canned `Vec<PickedFile>` (set
/// via [`Self::set_next_pick`]); logs each `saveFile` call for assertion via
/// [`Self::last_save`] / [`Self::take_saves`]. Resolves eagerly so tests stay
/// deterministic.
#[derive(Default, Clone)]
pub struct RecordingFilePicker {
    inner: std::sync::Arc<RecordingFilePickerInner>,
}

#[derive(Default)]
struct RecordingFilePickerInner {
    next_pick: std::sync::Mutex<Vec<PickedFile>>,
    saves: std::sync::Mutex<Vec<RecordedSave>>,
}

/// Simplified view of a `saveFile` call captured by [`RecordingFilePicker`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedSave {
    pub name: String,
    pub bytes: Vec<u8>,
}

impl RecordingFilePicker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-canned files returned by the next `pick(opts).await`. Returned
    /// unchanged on every subsequent pick until replaced.
    pub fn set_next_pick(&self, files: Vec<PickedFile>) {
        *self.inner.next_pick.lock().unwrap() = files;
    }

    /// Drain all `saveFile` calls logged so far, in insertion order.
    pub fn take_saves(&self) -> Vec<RecordedSave> {
        std::mem::take(&mut *self.inner.saves.lock().unwrap())
    }

    /// Drain all saves and return the last one.
    pub fn last_save(&self) -> Option<RecordedSave> {
        self.take_saves().pop()
    }
}

impl FilePickerBackend for RecordingFilePicker {
    fn pick(&self, _opts: PickOptions) -> Pin<Box<dyn Future<Output = Vec<PickedFile>>>> {
        let files = self.inner.next_pick.lock().unwrap().clone();
        Box::pin(std::future::ready(files))
    }
    fn save(
        &self,
        name: String,
        bytes: Vec<u8>,
        _opts: SaveOptions,
    ) -> Pin<Box<dyn Future<Output = ()>>> {
        self.inner
            .saves
            .lock()
            .unwrap()
            .push(RecordedSave { name, bytes });
        Box::pin(std::future::ready(()))
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
        (
            (self.left + self.right) / 2.0,
            (self.top + self.bottom) / 2.0,
        )
    }
}

/// Records shell-layer egress (cursor + text-input state) pushed by the
/// engine. Installed via `TurAppBuilder::shell` — the engine emits
/// `HostMsg::Shell(SetCursor)` and `HostMsg::Shell(RequestTextInput)`;
/// `apply_msg` applies them here.
/// Test `Shell`: records cursor + text-input egress into shared slots
/// (drained via `take_current_cursor` / `take_current_text_input_state`)
/// and carries the driver's manual vsync source as the frame clock
/// (handed to the engine at construction; `pump` fires it per frame).
struct RecordingShell {
    cursor_slot: std::sync::Arc<std::sync::Mutex<Option<Cursor>>>,
    text_input_slot: std::sync::Arc<std::sync::Mutex<Option<TextInputState>>>,
    vsync: Option<std::rc::Rc<dyn tur_engine::core::scheduler::VsyncSource>>,
}

impl tur_engine::Shell for RecordingShell {
    fn set_cursor(&mut self, cursor: Cursor) {
        *self.cursor_slot.lock().unwrap() = Some(cursor);
    }
    fn request_text_input(&mut self, state: TextInputState) {
        *self.text_input_slot.lock().unwrap() = Some(state);
    }
    fn take_vsync(&mut self) -> Option<std::rc::Rc<dyn tur_engine::core::scheduler::VsyncSource>> {
        self.vsync.take()
    }
}

/// Minimal [`tur_engine::Shell`] for raw-builder tests: no-op egress
/// (cursor / text-input requests dropped) + a caller-supplied frame
/// clock, handed to the engine at construction. Prefer [`TurTestApp`]
/// unless the test builds its own runtime; tests that inspect shell
/// egress define their own shell type (or use the harness's
/// `new_with_shell`).
pub struct TestShell {
    vsync: Option<std::rc::Rc<dyn tur_engine::core::scheduler::VsyncSource>>,
}

impl TestShell {
    /// Boxed shell carrying `vsync` as the frame clock — pass the
    /// driver's shared source so `driver.fire_vsync()` advances frames.
    pub fn new(vsync: std::rc::Rc<dyn tur_engine::core::scheduler::VsyncSource>) -> Box<Self> {
        Box::new(Self { vsync: Some(vsync) })
    }
}

impl tur_engine::Shell for TestShell {
    fn set_cursor(&mut self, _cursor: Cursor) {}
    fn request_text_input(&mut self, _state: TextInputState) {}
    fn take_vsync(&mut self) -> Option<std::rc::Rc<dyn tur_engine::core::scheduler::VsyncSource>> {
        self.vsync.take()
    }
}

pub struct TurTestApp {
    inner: Rc<TurApp>,
    /// The engine's deterministic clock. Advanced frame-by-frame by
    /// [`Self::wait_for`] / [`Self::wait_for_timeout`]. Shared with the
    /// engine `FrameEnv` and the boa `Context`, so `Date.now()` and timer
    /// scheduling see the same time.
    clock: std::sync::Arc<MutexFixedClock>,
    /// The scheduler driver's virtual clock. Advanced alongside `clock`
    /// so `sleep()` futures fire on the same virtual timeline.
    driver: Rc<TestSchedulerDriver>,
    /// Per-frame outcomes shipped by the engine's autonomous loop via the
    /// `after_frame` hook. `pump` awaits one item per vsync kick;
    /// `wait_for` / `wait_for_timeout` build on `pump`.
    frame_rx: RefCell<futures::channel::mpsc::UnboundedReceiver<FrameOutcome>>,
    cursor_slot: std::sync::Arc<std::sync::Mutex<Option<Cursor>>>,
    text_input_slot: std::sync::Arc<std::sync::Mutex<Option<TextInputState>>>,
    clipboard: RecordingClipboard,
    http: Option<RecordingHttp>,
    filepicker: Option<RecordingFilePicker>,
    synthetic_time_ms: u64,
}

impl TurTestApp {
    pub fn new(width: f64, height: f64) -> Result<Self, TurError> {
        Self::build(width, height, None, None, Vec::new(), None, None)
    }

    /// Construct with a custom [`Shell`] installed at construction time
    /// (replacing the default `RecordingShell`). Cursor /
    /// `take_current_cursor` / `take_current_text_input_state` recorders
    /// are absent — the supplied shell owns all egress observation.
    ///
    /// The factory receives the harness's vsync source and MUST embed it
    /// as the shell's frame clock (returned from `Shell::take_vsync`) —
    /// `pump` / `wait_for` advance frames by firing it.
    pub fn new_with_shell(
        width: f64,
        height: f64,
        shell_factory: impl FnOnce(
            std::rc::Rc<dyn tur_engine::core::scheduler::VsyncSource>,
        ) -> Box<dyn tur_engine::Shell>,
    ) -> Result<Self, TurError> {
        // Reserve the driver's source for the factory before `build`
        // constructs its own.
        let driver = TestSchedulerDriver::new();
        let shell = shell_factory(driver.vsync_source());
        Self::build_with_driver(
            width,
            height,
            None,
            None,
            Vec::new(),
            None,
            Some(shell),
            driver,
        )
    }

    /// Construct with `TurNetPlugin` registered against a fresh
    /// [`RecordingHttp`], so tests can drive `request()` from JS. Pre-canned
    /// responses via [`Self::set_http_response`]; capture requests via
    /// [`Self::last_http_request`].
    pub fn new_with_http(width: f64, height: f64) -> Result<Self, TurError> {
        Self::build(
            width,
            height,
            Some(RecordingHttp::new()),
            None,
            Vec::new(),
            None,
            None,
        )
    }

    /// Construct with `TurFilePickerPlugin` registered against a fresh
    /// [`RecordingFilePicker`], so tests can drive `filePicker.pick()` /
    /// `saveFile()` from JS. Pre-canned picks via [`Self::set_next_pick`];
    /// capture saves via [`Self::last_save`].
    pub fn new_with_filepicker(width: f64, height: f64) -> Result<Self, TurError> {
        Self::build(
            width,
            height,
            None,
            Some(RecordingFilePicker::new()),
            Vec::new(),
            None,
            None,
        )
    }

    /// Construct with additional plugins registered beyond the default
    /// `TurStdPlugin` + `TurClipboardPlugin`. Used by tests that need to
    /// inject extra modules (e.g. [`NativeModulePlugin`] for a test-only
    /// `tur:*` module).
    pub fn new_with_extra_plugins(
        width: f64,
        height: f64,
        extra_plugins: Vec<Box<dyn Plugin>>,
    ) -> Result<Self, TurError> {
        Self::build(width, height, None, None, extra_plugins, None, None)
    }

    /// Construct with a custom [`Renderer`] (instead of the default
    /// `NoopRenderer`), keeping every other harness ergonomic (load / wheel /
    /// render / element_tree). Used by tests that need to inspect the actual
    /// `RenderCommand` stream a frame produces (e.g. paint-walk culling).
    pub fn new_with_renderer(
        width: f64,
        height: f64,
        renderer: Box<dyn Renderer>,
    ) -> Result<Self, TurError> {
        Self::build(width, height, None, None, Vec::new(), Some(renderer), None)
    }

    #[allow(clippy::needless_pass_by_value)]
    fn build(
        width: f64,
        height: f64,
        http: Option<RecordingHttp>,
        filepicker: Option<RecordingFilePicker>,
        extra_plugins: Vec<Box<dyn Plugin>>,
        renderer: Option<Box<dyn Renderer>>,
        shell: Option<Box<dyn tur_engine::Shell>>,
    ) -> Result<Self, TurError> {
        Self::build_with_driver(
            width,
            height,
            http,
            filepicker,
            extra_plugins,
            renderer,
            shell,
            TestSchedulerDriver::new(),
        )
    }

    /// `build` with a caller-chosen driver — `new_with_shell` needs the
    /// driver BEFORE building so its factory can hand the shell the
    /// driver's vsync source (the harness fires it per driven frame).
    #[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
    fn build_with_driver(
        width: f64,
        height: f64,
        http: Option<RecordingHttp>,
        filepicker: Option<RecordingFilePicker>,
        extra_plugins: Vec<Box<dyn Plugin>>,
        renderer: Option<Box<dyn Renderer>>,
        shell: Option<Box<dyn tur_engine::Shell>>,
        driver: Rc<TestSchedulerDriver>,
    ) -> Result<Self, TurError> {
        let clipboard = RecordingClipboard::new();
        let clock = std::sync::Arc::new(MutexFixedClock::new(0));
        // Default pool: effectively uncapped → every harness app gets its
        // own dedicated lane thread (the historical threading).
        let worker_pool = WorkerPoolHandle::new("test", usize::MAX);
        let mut builder = TurRuntime::builder()
            .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
            .clock(clock.clone())
            .worker_spawner(driver.worker_spawner())
            .host_loop(driver.host_loop())
            .worker_pool(worker_pool.clone())
            .capability({
                let clip = clipboard.clone();
                move |_| Ok(Clipboard::new(clip))
            })
            .plugin(TurStdPlugin)
            .plugin(tur_animation::TurAnimationPlugin)
            .plugin(TurClipboardPlugin);
        if let Some(http_impl) = http.clone() {
            builder = builder
                .capability(move |_| Ok(Http::new(http_impl)))
                .plugin(TurNetPlugin);
        }
        if let Some(filepicker_impl) = filepicker.clone() {
            builder = builder
                .capability(move |_| Ok(FilePicker::new(filepicker_impl)))
                .plugin(TurFilePickerPlugin);
        }
        for p in extra_plugins {
            builder = builder.plugin_boxed(p);
        }
        let runtime = builder.build()?;
        let renderer: Box<dyn Renderer> = renderer.unwrap_or_else(|| Box::new(NoopRenderer::new()));
        // Shell egress is a per-instance host-side surface (see
        // `core::shell`), installed at construction exactly like an
        // embedder would. Default: a `RecordingShell` capturing cursor +
        // text-input state (drained via `take_current_cursor` /
        // `take_current_text_input_state`) and carrying the driver's
        // manual vsync source as the frame clock; tests pass their own
        // shell via `new_with_shell` (which must then carry a vsync
        // source itself).
        let cursor_slot = std::sync::Arc::new(std::sync::Mutex::new(None));
        let text_input_slot = std::sync::Arc::new(std::sync::Mutex::new(None));
        let shell: Box<dyn tur_engine::Shell> = shell.unwrap_or_else(|| {
            Box::new(RecordingShell {
                cursor_slot: cursor_slot.clone(),
                text_input_slot: text_input_slot.clone(),
                vsync: Some(driver.vsync_source()),
            })
        });
        let (inner, mut looper) = runtime
            .app_builder()
            .worker_pool(worker_pool)
            .renderer(renderer, (width, height), 1.0)
            .shell(shell)
            .build()?;
        // Drive the production autonomous loop (the same loop wasm/Android
        // drive via `TurAppLooper::run`). The `after_frame` hook ships each
        // `FrameOutcome` into `frame_rx`; `pump` pairs one `fire_vsync`
        // with one awaited outcome.
        let (frame_tx, frame_rx) = futures::channel::mpsc::unbounded::<FrameOutcome>();
        looper.set_after_frame_hook(Some(Rc::new(move |o| {
            let _ = frame_tx.unbounded_send(o);
        })));
        driver.spawn_local(Box::pin(looper.run()));
        let app = Self {
            inner,
            frame_rx: RefCell::new(frame_rx),
            clock,
            driver,
            cursor_slot,
            text_input_slot,
            clipboard,
            http,
            filepicker,
            synthetic_time_ms: 1_700_000_000_000,
        };
        // Bootstrap: the worker self-paints on load (the initial resize the
        // engine pushes in `app_builder().build(...)`); drive one frame so
        // the app is mounted before the test starts.
        let _ = app.pump();
        Ok(app)
    }

    /// Advance both the boa clock + the scheduler driver's virtual clock
    /// by `ms`. Sleep futures fire when the virtual clock reaches their
    /// deadline.
    fn advance_clock(&self, ms: u64) {
        self.clock.forward(ms);
        self.driver.advance(ms);
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

    /// Current synthetic-time stamp (ms), bumped via `bump_time`. Lets tests
    /// that push their own platform events with explicit `time_ms` reuse the
    /// same monotonically-increasing clock the high-level helpers use (so e.g.
    /// the gesture arena's slop + fling velocity math sees realistic times).
    pub fn last_synthetic_time_ms(&self) -> u64 {
        self.synthetic_time_ms
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
        // Case dist files are ES modules that import `tur:*` (resolved by
        // the engine's module loader) and satisfy the module lifecycle
        // contract natively: their own `start({ store })` calls `mount(view)`
        // against the engine-provided instance store.
        block_on(self.inner.backend().load_module(source.as_str()))?;
        // Drive the module's initial render to quiescence (frozen clock)
        // before the test starts interacting.
        self.wait_for_timeout(Duration::ZERO);
        Ok(())
    }

    /// Direct access to the underlying `TurApp` — lets a test register extra
    /// `__tur.*` / `__turHost.*` fns (e.g. a fake `__tur.request` backed by an
    /// in-process WebDAV server) before loading a bundle.
    pub fn with_app<R>(&self, f: impl FnOnce(&TurApp) -> R) -> R {
        f(&self.inner)
    }

    /// Direct reference to the underlying `TurApp` — for host-side APIs like
    /// `EventBus::of(app)` that need `&TurApp`.
    pub fn app(&self) -> &TurApp {
        &self.inner
    }

    /// Drive the production loop forward by exactly one frame: fire one
    /// vsync, then block (driving the `LocalSet` + the spawned loop)
    /// until the `after_frame` hook reports a completed frame. The worker
    /// pumps once per Wake; the loop renders + dispatches all side-effects
    /// (cursor / focus / images) via the shared `apply_msg`, so this path is
    /// identical to what wasm/Android drive. The single frame primitive every
    /// sync helper builds on.
    pub fn pump(&self) -> FrameOutcome {
        pump_one(&self.driver, &self.frame_rx)
    }

    /// The condition-wait primitive. Drives the loop one frame at a time,
    /// advancing the virtual clock by `FRAME_STEP_MS` per step, checking
    /// `predicate` after each drive — so time-based observables (a `sleep`
    /// resolving, an animation threshold) resolve as virtual time progresses.
    /// Always advances at least one frame before the first check, so it is
    /// safe right after a fire-and-forget input. Returns `true` once the
    /// predicate holds, or `false` after the cap (~2 s virtual). For a
    /// frozen-clock sync (no time advance), use `wait_for_timeout(ZERO)`.
    pub fn wait_for(&self, predicate: impl Fn(&TurTestApp) -> bool) -> bool {
        const CAP_MS: u64 = 2_000;
        let mut elapsed_ms: u64 = 0;
        loop {
            self.advance_clock(FRAME_STEP_MS);
            elapsed_ms += FRAME_STEP_MS;
            self.pump();
            if predicate(self) {
                return true;
            }
            if elapsed_ms >= CAP_MS {
                return false;
            }
        }
    }

    /// The time-advance primitive. Advances the virtual clock by `timeout` in
    /// `FRAME_STEP_MS` ticks, driving the loop to **quiescence** at each tick.
    /// `timeout == ZERO` is the pure quiescence form: drive frames at a
    /// frozen clock until the engine reports no immediately-available work —
    /// this is what event-syncs use, since it doesn't perturb time-sensitive
    /// assertions. Pure e2e model: only `wait_for` (sync to an observable,
    /// frozen clock) and `wait_for_timeout` (advance time + quiescence)
    /// drive the loop.
    pub fn wait_for_timeout(&self, timeout: Duration) {
        let total_ms = timeout.as_millis() as u64;
        let mut elapsed_ms: u64 = 0;
        loop {
            let step = FRAME_STEP_MS.min(total_ms.saturating_sub(elapsed_ms));
            self.advance_clock(step);
            elapsed_ms += step;
            // Drive to quiescence at this clock tick (cap 8 frames per tick).
            for _ in 0..8 {
                let outcome = self.pump();
                if !outcome.painted && outcome.schedule == NextFrame::Idle {
                    break;
                }
            }
            if elapsed_ms >= total_ms {
                break;
            }
        }
    }

    /// Fire-and-forget: push a viewport resize. Driven by a subsequent
    /// `wait_for` (exercising the full relayout path).
    pub fn resize(&mut self, width: f64, height: f64) {
        self.inner.push_platform_event(ShellEvent::Resize {
            logical_width: width as u32,
            logical_height: height as u32,
            dpr: 1.0,
        });
    }

    /// Snapshot of the live element tree, built on the worker via the
    /// `with_tree` escape hatch. Returns an owned value (not a `Ref`) —
    /// the live tree lives on the worker thread; main can only see
    /// snapshots of it. Tests should call this once per `render()` /
    /// input step they want to inspect (the snapshot is not
    /// auto-refreshed).
    pub fn element_tree(&self) -> NodeTreeSnapshot {
        self.with_tree(|tree, _focus| tree.tree_snapshot())
            .expect("worker gone")
    }

    /// Fire-and-forget: push a pointer-down + pointer-up (a full click) onto
    /// the platform-event queue. The gesture is recognized when a subsequent
    /// `wait_for` / `wait_for_timeout` drives the loop. Both events carry the
    /// same `time_ms` (bumped once) so the gesture composer classifies a tap.
    pub fn click(&mut self, x: f64, y: f64) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerDown {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
                device: PointerDeviceKind::Mouse,
            }));
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerUp {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
                device: PointerDeviceKind::Mouse,
            }));
    }

    /// Fire-and-forget: push a key-down event. Driven by a subsequent
    /// `wait_for` / `wait_for_timeout`.
    pub fn send_key(&mut self, key: &str) {
        self.inner.push_platform_event(ShellEvent::Key(KeyEvent {
            key: key.to_string(),
            code: key.to_string(),
            modifiers: Modifiers::default(),
            event_type: KeyEventType::Down,
        }));
    }

    /// Fire-and-forget: push an IME event. Driven by a subsequent `wait_for`.
    pub fn send_ime(&mut self, event: ImeEvent) {
        self.inner.push_platform_event(ShellEvent::Ime(event));
    }

    pub fn send_key_with_modifiers(&mut self, key: &str, shift: bool, ctrl: bool) {
        self.send_key_with_modifiers_full(key, shift, ctrl, false);
    }

    /// Full-key modifier helper. `meta` covers Cmd on macOS / Win on Windows.
    /// Use this for Cmd+C / Cmd+V / Cmd+S tests. Fire-and-forget.
    pub fn send_key_with_modifiers_full(&mut self, key: &str, shift: bool, ctrl: bool, meta: bool) {
        self.inner.push_platform_event(ShellEvent::Key(KeyEvent {
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
    }

    /// Fire-and-forget: push a pointer-down (left button). Driven by a
    /// subsequent `wait_for` / `wait_for_timeout`.
    pub fn pointer_down(&mut self, x: f64, y: f64) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerDown {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
                device: PointerDeviceKind::Mouse,
            }));
    }

    /// Simulate a double-click at `(x, y)`. Two `pointer_down`s are pushed in
    /// quick succession (40 ms apart, well inside the engine's 500 ms window)
    /// at the same position, so the gesture composer classifies the second
    /// one as `PointerDoubleDown`. Fire-and-forget.
    pub fn double_click(&mut self, x: f64, y: f64) {
        self.pointer_down(x, y);
        self.pointer_down(x, y);
    }

    /// Simulate a triple-click at `(x, y)`. Three `pointer_down`s in quick
    /// succession — the third one is classified as `PointerTripleDown`.
    /// Fire-and-forget.
    pub fn triple_click(&mut self, x: f64, y: f64) {
        self.pointer_down(x, y);
        self.pointer_down(x, y);
        self.pointer_down(x, y);
    }

    /// Fire-and-forget: push a pointer-move. Driven by a subsequent `wait_for`.
    pub fn pointer_move(&mut self, x: f64, y: f64) {
        let time_ms = self.synthetic_time_ms;
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerMove {
                position: Offset::new(x, y),
                device: PointerDeviceKind::Mouse,
                time_ms,
            }));
    }

    /// Fire-and-forget: push a pointer-up (left button). Driven by a
    /// subsequent `wait_for`.
    pub fn pointer_up(&mut self, x: f64, y: f64) {
        let time_ms = self.synthetic_time_ms;
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerUp {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                device: PointerDeviceKind::Mouse,
                time_ms,
            }));
    }

    /// Same as `pointer_down` but with an explicit mouse button. Used to
    /// simulate right-click (button 2) without an enclosing `click` gesture.
    /// Fire-and-forget.
    pub fn pointer_down_with_button(&mut self, x: f64, y: f64, button: MouseButton) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerDown {
                position: Offset::new(x, y),
                button,
                time_ms,
                device: PointerDeviceKind::Mouse,
            }));
    }

    /// Fire-and-forget: push a pointer-up with an explicit button.
    pub fn pointer_up_with_button(&mut self, x: f64, y: f64, button: MouseButton) {
        let time_ms = self.synthetic_time_ms;
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerUp {
                position: Offset::new(x, y),
                button,
                device: PointerDeviceKind::Mouse,
                time_ms,
            }));
    }

    /// Push a right-click sequence: pointer-down(button=Right) then
    /// pointer-up(button=Right). The engine's gesture arena derives the
    /// `ContextMenu` gesture from the right-button pointer-up — there is no
    /// separate context-menu platform event anymore. Fire-and-forget.
    pub fn right_click(&mut self, x: f64, y: f64) {
        self.pointer_down_with_button(x, y, MouseButton::Right);
        self.pointer_up_with_button(x, y, MouseButton::Right);
    }

    /// Fire-and-forget: push a wheel event. Driven by a subsequent `wait_for`.
    pub fn wheel(&mut self, delta_x: f64, delta_y: f64, x: f64, y: f64) {
        self.inner.push_platform_event(ShellEvent::Wheel {
            delta_x,
            delta_y,
            position: Offset::new(x, y),
        });
    }

    /// Simulate a touch drag from `start` to `end` in `steps` moves, advancing
    /// the deterministic clock by one frame (`FRAME_STEP_MS`) before each move
    /// so every event carries a distinct, increasing `time_ms` (matching how a
    /// real browser stamps `event.timeStamp`). Ends with a touch-up. The whole
    /// sequence is pushed fire-and-forget; a subsequent `wait_for` /
    /// `wait_for_timeout` drives it.
    pub fn touch_drag(&mut self, start: (f64, f64), end: (f64, f64), steps: usize) {
        let time_ms = self.clock.now().millis_since_epoch();
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerDown {
                position: Offset::new(start.0, start.1),
                button: MouseButton::Left,
                time_ms,
                device: PointerDeviceKind::Touch,
            }));
        for i in 1..=steps {
            self.advance_clock(FRAME_STEP_MS);
            let t = i as f64 / steps as f64;
            let x = start.0 + (end.0 - start.0) * t;
            let y = start.1 + (end.1 - start.1) * t;
            let time_ms = self.clock.now().millis_since_epoch();
            self.inner
                .push_platform_event(ShellEvent::Pointer(PointerInput::PointerMove {
                    position: Offset::new(x, y),
                    device: PointerDeviceKind::Touch,
                    time_ms,
                }));
        }
        self.advance_clock(FRAME_STEP_MS);
        let time_ms = self.clock.now().millis_since_epoch();
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerUp {
                position: Offset::new(end.0, end.1),
                button: MouseButton::Left,
                device: PointerDeviceKind::Touch,
                time_ms,
            }));
    }

    /// Push a touch pointer-down with an explicit event `time_ms`
    /// (fire-and-forget). For the batched-moves fling regression test, which
    /// pushes a whole down→moves→up sequence carrying distinct real timestamps
    /// but drains it in a single `wait_for` (simulating a mobile browser
    /// coalescing several touchmoves into one frame).
    pub fn push_touch_down(&mut self, x: f64, y: f64, time_ms: u64) {
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerDown {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
                device: PointerDeviceKind::Touch,
            }));
    }

    /// Push a touch pointer-move with an explicit event `time_ms`
    /// (fire-and-forget). See [`Self::push_touch_down`].
    pub fn push_touch_move(&mut self, x: f64, y: f64, time_ms: u64) {
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerMove {
                position: Offset::new(x, y),
                device: PointerDeviceKind::Touch,
                time_ms,
            }));
    }

    /// Push a touch pointer-up with an explicit event `time_ms`
    /// (fire-and-forget). See [`Self::push_touch_down`].
    pub fn push_touch_up(&mut self, x: f64, y: f64, time_ms: u64) {
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerUp {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                device: PointerDeviceKind::Touch,
                time_ms,
            }));
    }

    /// Fire-and-forget: push a touch pointer-down at `(x, y)`. Driven by a
    /// subsequent `wait_for`.
    pub fn touch_down(&mut self, x: f64, y: f64) {
        let time_ms = self.bump_time(40);
        self.inner
            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerDown {
                position: Offset::new(x, y),
                button: MouseButton::Left,
                time_ms,
                device: PointerDeviceKind::Touch,
            }));
    }

    pub fn has_click_handler(&self, id: ElementNodeId) -> bool {
        use tur_engine::builtin_plugins::gesture::PointerInteractElement;
        self.with_element(id, |e| {
            e.cast::<PointerInteractElement>()
                .map(|p| p.has_on_click())
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }

    pub fn has_mouse_region_callbacks(&self, id: ElementNodeId) -> bool {
        use tur_engine::builtin_plugins::gesture::MouseRegionElement;
        self.with_element(id, |e| {
            e.cast::<MouseRegionElement>()
                .map(|m| m.has_region_callbacks())
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }

    pub fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        let key_owned: Vec<String> = key.iter().map(|s| s.to_string()).collect();
        self.with_tree(move |tree, _focus| {
            let refs: Vec<&str> = key_owned.iter().map(String::as_str).collect();
            tree.query_element(&refs)
        })
        .flatten()
    }

    /// Absolute bounds of the node (translation of its world affine +
    /// painted size), from a `DevNodeData` snapshot built on the worker.
    pub fn get_element_absolute_bounds(&self, id: ElementNodeId) -> Option<Rect> {
        let node = self.dev_tool_get_element(id.into())?;
        let (x, y) = node.absolute;
        let (w, h) = node.size;
        Some(Rect {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        })
    }

    pub fn focused_element(&self) -> Option<ElementNodeId> {
        self.with_tree(|_tree, focus| focus.focused()).flatten()
    }

    /// Logical-space `(x, y, w, h)` of the focused element's caret —
    /// ancestor-offset accumulation plus the element's own
    /// `cursor_rect_relative`, computed on the worker via `with_tree`.
    pub fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        self.with_tree(|tree, focus| {
            let focused_id = focus.focused()?;
            let mut abs_x = 0.0f64;
            let mut abs_y = 0.0f64;
            let mut current = Some(NodeId::from(focused_id));
            while let Some(id) = current {
                let node = tree.get_element(ElementNodeId::new(id.as_u64()))?;
                abs_x += node.computed_layout.offset.x;
                abs_y += node.computed_layout.offset.y;
                current = node.parent;
            }
            let node = tree.get_element(focused_id)?;
            let element = node.element.as_ref()?;
            let (cx, cy, cw, ch) = element.cursor_rect_relative()?;
            Some((abs_x + cx, abs_y + cy, cw, ch))
        })
        .flatten()
    }

    /// Inspect an element's internal state via a closure. The closure runs
    /// on the worker thread (where the live `AnyElement` lives), so it can
    /// do typed introspection that isn't serializable across the thread
    /// boundary — `e.cast::<TextElement>().map(|t| t.spans())`, etc.
    ///
    /// Constraints:
    /// - `R: Send + 'static` — the result crosses worker→main.
    /// - `cb: Send + 'static` — the closure crosses main→worker.
    ///
    /// Engine-side this is a test-only surface: it's built on
    /// [`Self::with_tree`] (the `NodeTreeData` + `FocusManager` escape
    /// hatch), which is the engine's only test RPC.
    pub fn with_element<R: Send + 'static>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R + Send + 'static,
    ) -> Option<R> {
        self.with_tree(move |tree, _focus| {
            tree.get_element(id)
                .and_then(|node| node.element.as_ref())
                .map(cb)
        })
        .flatten()
    }

    /// Like [`Self::with_element`], but the closure receives the whole
    /// live `NodeTreeData` AND `FocusManager` — the general test-only
    /// escape hatch (reconstructs the former per-field focused/dev-tool
    /// queries). Same `Send + 'static` constraints.
    pub fn with_tree<R: Send + 'static>(
        &self,
        cb: impl FnOnce(&NodeTreeData, &tur_engine::core::focus::FocusManager) -> R + Send + 'static,
    ) -> Option<R> {
        block_on(self.inner.with_tree(cb))
    }

    /// Returns the most recent cursor pushed by the engine since the last
    /// call. The engine pushes cursor changes through the host-side
    /// `RecordingShell` via `HostMsg::Shell(SetCursor)`; this drains that recording.
    pub fn take_current_cursor(&self) -> Option<Cursor> {
        self.cursor_slot.lock().unwrap().take()
    }

    /// Returns the most recent text-input state pushed by the engine since
    /// the last call. The engine pushes text-input changes through the
    /// host-side `RecordingShell` via `HostMsg::Shell(RequestTextInput)`;
    /// this drains that recording.
    pub fn take_current_text_input_state(&self) -> Option<TextInputState> {
        self.text_input_slot.lock().unwrap().take()
    }

    /// Drain any text written to the clipboard via `AppEvent::Custom`
    /// carrying a `ClipboardWriteEvent` (e.g. EditableText's Cmd+C / Cmd+X
    /// handling) since the last call. Returns the latest write (the
    /// `RecordingClipboard` logs every write; this drains all and returns
    /// the last, matching the old slot semantics).
    pub fn take_clipboard_write(&self) -> Option<String> {
        self.clipboard.last_write()
    }

    /// Condition-wait variant of [`Self::take_clipboard_write`]: drive
    /// frames until a clipboard write lands, returning the latest one.
    ///
    /// The write is spawned on the worker's own executor
    /// (`WorkerContext::spawn_local` in `ClipboardWriteSubsystem`), which
    /// the lane thread polls **after** the frame reply is sent — a single
    /// `wait_for_timeout(ZERO)` therefore gives the test no happens-before
    /// edge over the poll, and under CPU contention (parallel test runs)
    /// the raw `take_clipboard_write` can observe `None` before the write
    /// lands. Polling here is the deterministic sync point. Panics if no
    /// write arrives within `wait_for`'s cap (~2 s virtual).
    pub fn wait_for_clipboard_write(&self) -> String {
        let last = std::cell::RefCell::new(None);
        assert!(
            self.wait_for(|app| {
                *last.borrow_mut() = app.take_clipboard_write();
                last.borrow().is_some()
            }),
            "timed out waiting for a clipboard write"
        );
        last.into_inner()
            .expect("wait_for returned true without a write")
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

    /// Pre-canned streaming response for the next `requestStream(opts).await`.
    /// Panics if this app wasn't constructed via [`Self::new_with_http`].
    pub fn set_http_stream(&self, status: u16, chunks: Vec<Vec<u8>>) {
        self.http
            .as_ref()
            .expect("TurTestApp::set_http_stream requires new_with_http")
            .set_next_stream(status, chunks);
    }

    /// The most recent request seen by the recording, or `None` if no
    /// request has been issued. Panics if not constructed with http.
    pub fn last_http_request(&self) -> Option<RecordedRequest> {
        self.http
            .as_ref()
            .expect("TurTestApp::last_http_request requires new_with_http")
            .last_request()
    }

    /// Pre-canned files for the next `filePicker.pick()` call from JS. Panics
    /// if this app wasn't constructed via [`Self::new_with_filepicker`].
    pub fn set_next_pick(&self, files: Vec<PickedFile>) {
        self.filepicker
            .as_ref()
            .expect("TurTestApp::set_next_pick requires new_with_filepicker")
            .set_next_pick(files);
    }

    /// The most recent `saveFile(name, bytes)` captured by the recording, or
    /// `None` if none. Panics if not constructed with filepicker.
    pub fn last_save(&self) -> Option<RecordedSave> {
        self.filepicker
            .as_ref()
            .expect("TurTestApp::last_save requires new_with_filepicker")
            .last_save()
    }

    /// Push a synthetic paste event — equivalent to the embedder firing
    /// `paste` on the hidden textarea. The paste is wrapped as a
    /// `ClipboardPlatformPasteEvent` (PlatformEvent::Custom); tur-clipboard's
    /// `ClipboardPlatformSubsystem` re-emits it as a `ClipboardPasteEvent`
    /// (AppEvent::Custom), and tur-text's `ClipboardPasteSubsystem` then
    /// inserts `text` into the focused editable.
    pub fn push_paste_event(&mut self, text: &str) {
        self.inner
            .push_platform_event(tur_engine::platform_paste(text.to_string()));
    }

    pub fn eval_js(&self, source: &str) -> String {
        // RPC to the worker: it runs `ctx.eval(source)`, drains jobs, and
        // replies with the display string. Synchronous from the test's POV
        // (blocks on the worker's reply via `block_on`).
        block_on(self.inner.backend().eval_js(source))
    }

    /// Evaluate `source` as an ES module and invoke its `start()` export
    /// (the module lifecycle contract) — supports real
    /// `import { … } from "tur:std"` (or `tur-ext/demo-helper`/`tur:net`). Returns
    /// nothing; read results back via [`eval_js`](Self::eval_js).
    ///
    /// Legacy fixture adapter: inline test bundles predate the lifecycle
    /// contract, so a source that doesn't already export `start` is
    /// auto-wrapped (imports hoisted, remaining statements moved into
    /// `export function start() { … }`). Contract tests use
    /// [`load_module_raw`](Self::load_module_raw) for the strict path.
    pub fn eval_module_source(&self, source: &str) -> Result<(), TurError> {
        let wrapped = wrap_legacy_start(source);
        block_on(self.inner.backend().load_module(wrapped.as_str())).map_err(TurError::from)
    }

    /// Strict module-lifecycle path: loads `source` verbatim — it MUST
    /// export `function start()` (missing/invalid `start` fails the load).
    pub fn load_module_raw(&self, source: &str) -> Result<(), TurError> {
        block_on(self.inner.backend().load_module(source)).map_err(TurError::from)
    }

    /// Structured dev-tool snapshot of the root node, or `None` if no root
    /// is mounted (pre-first-mount / post-teardown). Children are bare ids;
    /// iterate with `dev_tool_get_element`.
    pub fn dev_tool_element_tree(&self) -> Option<tur_engine::core::elements::DevNodeData> {
        self.with_tree(|tree, _focus| {
            tree.root_element_id()
                .and_then(|root| tree.dev_tool_node(root.into()))
        })
        .flatten()
    }

    /// Structured dev-tool snapshot of an arbitrary node by id.
    pub fn dev_tool_get_element(
        &self,
        id: NodeId,
    ) -> Option<tur_engine::core::elements::DevNodeData> {
        self.with_tree(move |tree, _focus| tree.dev_tool_node(id))
            .flatten()
    }
}

/// Single-frame pump shared by `TurTestApp::pump` and `RawAppLooper`:
/// drain stale outcomes the worker produced between drives, fire one
/// vsync, then block until the `after_frame` hook reports a completed
/// frame.
fn pump_one(
    driver: &TestSchedulerDriver,
    frame_rx: &RefCell<futures::channel::mpsc::UnboundedReceiver<FrameOutcome>>,
) -> FrameOutcome {
    use futures::future::FutureExt;
    // Drain stale outcomes the worker produced between drives (it
    // self-wakes via `wake_if_dirty` whenever flush leaves paint-worthy
    // state). Without this drain, a stale frame would be consumed instead
    // of the fresh one that processes currently-queued events.
    while let Some(Some(_stale)) = frame_rx.borrow_mut().next().now_or_never() {}
    driver.fire_vsync();
    driver
        .block_on(frame_rx.borrow_mut().next())
        .expect("worker destroyed mid-frame")
}

/// Run-loop driver for tests that hold a raw `Rc<TurApp>` (multi-instance,
/// custom runtime). Mirrors `TurTestApp`'s driving: spawns the production
/// autonomous loop once, installs an `after_frame` hook feeding a frame
/// channel, and exposes the same `pump` / `wait_for` / `wait_for_timeout`
/// primitives. Construct one per app instance.
pub struct RawAppLooper {
    app: Rc<TurApp>,
    driver: Rc<TestSchedulerDriver>,
    frame_rx: RefCell<futures::channel::mpsc::UnboundedReceiver<FrameOutcome>>,
}

impl RawAppLooper {
    /// Spawn the loop for `app` (driven by `driver`) and bootstrap one
    /// frame. `looper` is the app's built
    /// [`TurAppLooper`](tur_engine::TurAppLooper) — passed separately since
    /// the app handle alone no longer carries it.
    pub fn new(
        app: Rc<TurApp>,
        looper: tur_engine::TurAppLooper,
        driver: Rc<TestSchedulerDriver>,
    ) -> Self {
        let (frame_tx, frame_rx) = futures::channel::mpsc::unbounded::<FrameOutcome>();
        let mut looper = looper;
        looper.set_after_frame_hook(Some(Rc::new(move |o| {
            let _ = frame_tx.unbounded_send(o);
        })));
        driver.spawn_local(Box::pin(looper.run()));
        let looper = Self {
            app,
            driver,
            frame_rx: RefCell::new(frame_rx),
        };
        let _ = looper.pump();
        looper
    }

    pub fn app(&self) -> &TurApp {
        &self.app
    }

    /// Drive the loop forward by exactly one frame (see [`TurTestApp::pump`]).
    pub fn pump(&self) -> FrameOutcome {
        pump_one(&self.driver, &self.frame_rx)
    }

    /// Drive frames at a frozen clock until `predicate` holds (cap ~2 s). The
    /// predicate reads observable state via its captures.
    pub fn wait_for(&self, predicate: impl Fn() -> bool) -> bool {
        const CAP_FRAMES: usize = 125;
        for _ in 0..CAP_FRAMES {
            self.pump();
            if predicate() {
                return true;
            }
        }
        false
    }

    /// `wait_for_timeout`-equivalent: advance the driver's frame loop (no
    /// virtual clock here — raw apps own their own clock). `ZERO` drives to
    /// quiescence.
    pub fn wait_for_timeout(&self, timeout: Duration) {
        let frames = (timeout.as_millis() as u64).div_ceil(16);
        let iters = frames.max(1);
        for _ in 0..iters {
            // drive to quiescence at this tick (cap 8).
            for _ in 0..8 {
                let outcome = self.pump();
                if !outcome.painted && outcome.schedule == NextFrame::Idle {
                    break;
                }
            }
        }
    }
}
