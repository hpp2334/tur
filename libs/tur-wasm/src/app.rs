use crate::fonts::WasmFontLoader;
use boa_engine::context::time::{Clock, JsInstant};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use tur_clipboard_wasm::{Clipboard, TurClipboardPlugin, WasmClipboard};
use tur_engine::TurApp;
use tur_engine::core::layout::Offset;
use tur_engine::core::platform::key_event::{KeyEvent, KeyEventType, Modifiers};
use tur_engine::core::platform::{ImeEvent, PointerInput};
use tur_engine::core::scheduler::WorkerPoolHandle;
use tur_engine::core::shell::ShellEvent;
use tur_engine::renderer::vello::WebGlVelloRenderer;
use tur_filepicker_wasm::{FilePicker, TurFilePickerPlugin, WasmFilePicker};
use tur_net_wasm::{Http, TurNetPlugin, WasmHttp};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;

/// Engine `Clock` for wasm. boa's `StdClock` panics on
/// `wasm32-unknown-unknown` (`SystemTime::now()` is unimplemented), and
/// `std::time::Instant::now()` is unsupported too, so this reads
/// `Date.now()` — the same wall-clock source the old frame loop used for its
/// per-frame delta. Production thus gets live real time with no manual clock
/// forwarding. (Not strictly monotonic across system-clock adjustments, but
/// the engine's animation/timer math derives durations from deltas, which is
/// robust to the rare jump.)
#[derive(Default)]
struct WasmClock;

impl Clock for WasmClock {
    fn now(&self) -> JsInstant {
        let ms = js_sys::Date::now();
        let secs = (ms / 1000.0) as u64;
        let nanos = ((ms % 1000.0) * 1_000_000.0) as u32;
        JsInstant::new(secs, nanos)
    }

    fn system_time_millis(&self) -> i64 {
        js_sys::Date::now() as i64
    }
}

struct WasmState {
    app: Rc<TurApp>,
    _canvas: web_sys::HtmlCanvasElement,
    textarea: web_sys::HtmlTextAreaElement,
    is_composing: Cell<bool>,
    _resize_closure: Closure<dyn Fn()>,
    _resize_observer: web_sys::ResizeObserver,
    _pointer_down_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _pointer_up_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _pointer_move_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _wheel_closure: Closure<dyn Fn(web_sys::WheelEvent)>,
    _context_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _touch_start_closure: Closure<dyn Fn(web_sys::TouchEvent)>,
    _touch_move_closure: Closure<dyn Fn(web_sys::TouchEvent)>,
    _touch_end_closure: Closure<dyn Fn(web_sys::TouchEvent)>,
    _touch_cancel_closure: Closure<dyn Fn(web_sys::TouchEvent)>,
    _keydown_closure: Closure<dyn Fn(web_sys::KeyboardEvent)>,
    _keyup_closure: Closure<dyn Fn(web_sys::KeyboardEvent)>,
    _compositionstart_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _compositionupdate_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _compositionend_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _paste_closure: Closure<dyn Fn(web_sys::ClipboardEvent)>,
}

/// Embedder-side `Shell`: the engine pushes cursor and text-input
/// requests here during the frame loop. Cursor goes to the canvas CSS;
/// text-input positions the hidden `<textarea>` for IME composition.
///
/// Also carries the window's frame clock (rAF) — the engine takes it once
/// at construction via `take_vsync`.
///
/// Holds a `Weak<RefCell<...>>` into the wasm state so there is no
/// reference cycle — `request_text_input` is a no-op if the state has
/// been dropped.
struct WasmShell {
    canvas: web_sys::HtmlCanvasElement,
    /// Weak ref into the wasm state for textarea focus / caret positioning.
    state_weak: std::rc::Weak<std::cell::RefCell<Option<WasmState>>>,
    /// The window's frame clock, handed to the engine at construction.
    vsync: Option<Rc<crate::scheduler::WasmVsyncSource>>,
}

// SAFETY: `WasmShell` is only ever accessed from the wasm main thread
// (it's installed via `.shell()` at build time and invoked from
// `HostMsg::Shell` inside main's pump). The `HtmlCanvasElement` isn't
// `Send`/`Sync` on wasm32 because it wraps a raw `*mut` JsValue, but
// our usage is single-threaded so the unsafe impl is sound.
unsafe impl Send for WasmShell {}
unsafe impl Sync for WasmShell {}

impl tur_engine::Shell for WasmShell {
    fn set_cursor(&mut self, cursor: tur_engine::core::shell::Cursor) {
        let _ = self.canvas.style().set_property("cursor", cursor.as_str());
    }

    fn request_text_input(&mut self, state: tur_engine::core::shell::TextInputState) {
        let Some(s) = self.state_weak.upgrade() else {
            return;
        };
        let mut guard = s.borrow_mut();
        let Some(wasm) = guard.as_mut() else {
            return;
        };
        if state.is_editable {
            let _ = wasm.textarea.focus();
            if let Some((x, y, _w, _h)) = state.cursor_rect {
                let _ = wasm
                    .textarea
                    .style()
                    .set_property("left", &format!("{x}px"));
                let _ = wasm.textarea.style().set_property("top", &format!("{y}px"));
            }
        }
    }

    fn take_vsync(&mut self) -> Option<Rc<dyn tur_engine::core::scheduler::VsyncSource>> {
        self.vsync
            .take()
            .map(|v| v as Rc<dyn tur_engine::core::scheduler::VsyncSource>)
    }
}

/// Configuration for building a shared wasm tur runtime via
/// [`WasmRuntime::create`].
///
/// `tur-wasm` is a reusable embedder lib (no playground-plugin code):
/// the host cdylib supplies the engine-customization callback (extra plugins)
/// while `tur-wasm` owns all the generic capability backends.
pub struct WasmRuntimeConfig {
    /// Customize the [`tur_engine::TurRuntimeBuilder`] before `build()` — the
    /// caller adds its own plugins (and may override the default capabilities).
    /// `tur-wasm` has already registered the standard plugin set + clipboard /
    /// http / filepicker / cursor backends before invoking this.
    pub configure: Box<dyn FnOnce(tur_engine::TurRuntimeBuilder) -> tur_engine::TurRuntimeBuilder>,
    /// Extra worker pools to register (in addition to the built-in
    /// effectively-uncapped `default` pool every app falls back to).
    /// Declare a capped pool here (e.g. `WorkerPoolHandle::new("daemon", 2)`)
    /// and assign it per-app via [`WasmAppConfig::worker_pool`] so heavy background
    /// apps share workers without stalling the UI pool.
    pub worker_pools: Vec<WorkerPoolHandle>,
}

/// The shared wasm runtime — created once via [`WasmRuntime::create`]. Owns the
/// [`tur_engine::TurRuntime`] (fonts, clock, capabilities, plugins). Spawn
/// isolated instances (each with its own canvas/DOM or headless) via
/// [`WasmApp::create`] (which internally calls
/// [`TurRuntime::app_builder`](tur_engine::TurRuntime::app_builder)).
pub struct WasmRuntime {
    runtime: Rc<tur_engine::TurRuntime>,
    /// The built-in effectively-uncapped pool assigned to apps that don't
    /// pick one explicitly via [`WasmAppConfig::worker_pool`]. Registered on the
    /// engine runtime alongside `WasmRuntimeConfig::pools`.
    default_worker_pool: WorkerPoolHandle,
}

impl WasmRuntime {
    /// Build the shared runtime with the wasm-default capabilities (WasmClock,
    /// WasmFontLoader, WasmClipboard, WasmHttp, WasmFilePicker) + the standard
    /// plugin set, then apply the embedder's `configure` callback (extra
    /// plugins / capability overrides). No canvas/DOM — instances are spawned
    /// separately.
    pub fn create(cfg: WasmRuntimeConfig) -> Result<Self, JsValue> {
        // Architecture: the engine runs on a Web Worker (booted by the
        // in-tree `worker_spawn` module — a `SharedArrayBuffer`-backed
        // factory-message scheme). The WebGL renderer stays on the host
        // thread (web-sys types are realm-local); the worker ships
        // `Vec<RenderCommand>` batches to the host thread each frame and
        // `HostBackend` applies them to the renderer.
        //
        // Build-side config (in `.cargo/config.toml` + `rust-toolchain.toml`
        // + `[profile.wasm-dev]` in workspace `Cargo.toml`):
        //   • nightly toolchain (`nightly-2026-07-15`)
        //   • `-Z build-std=panic_abort,std`
        //   • `+atomics,+bulk-memory,+mutable-globals` target feature
        //   • `--shared-memory` + `--import-memory` + `--max-memory=1GiB`
        //   • `--export=__tls_*` / `__wasm_init_tls` (thread-id injection)
        //
        // No JS-side `initThreadPool(n)` is required: workers spawn on
        // demand from Rust (driven by `HostBackend::new`).

        let worker_spawner = crate::scheduler::WasmWorkerSpawner::new();
        let host_loop = Rc::new(crate::scheduler::WasmHostLoop);
        // Built-in default pool: effectively uncapped → one dedicated Web
        // Worker per app (the historical behavior) unless the embedder
        // assigns a capped pool per-app via `WasmAppConfig::worker_pool`.
        let default_worker_pool = WorkerPoolHandle::new("default", usize::MAX);
        let builder = tur_engine::TurRuntime::builder()
            .worker_spawner(worker_spawner)
            .host_loop(host_loop)
            .font_loader(std::sync::Arc::new(WasmFontLoader::new()))
            .clock(std::sync::Arc::new(WasmClock))
            .worker_pool(default_worker_pool.clone())
            .capability(|_| Ok(Clipboard::new(WasmClipboard)))
            .capability(|_| Ok(Http::new(WasmHttp)))
            .capability(|_| Ok(FilePicker::new(WasmFilePicker)))
            .plugin(tur_engine::TurStdPlugin)
            .plugin(tur_animation::TurAnimationPlugin)
            .plugin(TurClipboardPlugin)
            .plugin(TurNetPlugin)
            .plugin(TurFilePickerPlugin);
        // Let the embedder add its own plugins / override capabilities.
        let mut builder = (cfg.configure)(builder);
        for pool in cfg.worker_pools {
            builder = builder.worker_pool(pool);
        }
        let runtime = builder
            .build()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self {
            runtime,
            default_worker_pool,
        })
    }

    /// Access the underlying [`tur_engine::TurRuntime`] (for spawning raw
    /// instances outside the wasm DOM-wired helpers).
    pub fn runtime(&self) -> &Rc<tur_engine::TurRuntime> {
        &self.runtime
    }

    /// The built-in effectively-uncapped pool assigned to apps that don't
    /// pick one explicitly. Embedders building raw instances via
    /// [`Self::runtime`] can assign it (or a pool from
    /// [`WasmRuntimeConfig::pools`]) via `TurAppBuilder::worker_pool`.
    pub fn default_worker_pool(&self) -> &WorkerPoolHandle {
        &self.default_worker_pool
    }
}

/// Configuration for building a DOM-wired wasm app instance via
/// [`WasmApp::create`].
pub struct WasmAppConfig {
    /// `None` ⇒ full-viewport canvas (own wrapper `div`); `Some(id)` ⇒ embed
    /// the canvas inside the element with that id.
    pub container_id: Option<String>,
    /// The worker pool this app's engine worker is spawned into. `None` ⇒
    /// the runtime's built-in effectively-uncapped `default` pool (one
    /// dedicated Web Worker per app). Assign a capped pool registered via
    /// [`WasmRuntimeConfig::worker_pools`] to share workers between apps of
    /// the same group.
    pub worker_pool: Option<WorkerPoolHandle>,
}

/// Owning handle to a running wasm tur app instance. Built via
/// [`WasmApp::create`] from a [`WasmRuntime`] + [`WasmAppConfig`]. The embedder
/// cdylib wraps this in its own `#[wasm_bindgen]` struct (e.g.
/// `TurWebsiteApp`) and delegates its exported methods here — `tur-wasm` itself
/// exports no `#[wasm_bindgen]` surface.
#[derive(Clone)]
pub struct WasmApp {
    state: Rc<RefCell<Option<WasmState>>>,
}

trait JsResult<T> {
    fn err_to_jsval(self) -> Result<T, JsValue>;
}

impl<T, E: Into<JsValue>> JsResult<T> for Result<T, E> {
    fn err_to_jsval(self) -> Result<T, JsValue> {
        self.map_err(Into::into)
    }
}

/// Translate a DOM `MouseEvent.button` (+ ctrlKey state) into our
/// [`MouseButton`]. On macOS, a context-menu is triggered by Ctrl+click,
/// which the browser reports as `button=0` (primary) with `ctrlKey=true`;
/// normalize that to [`MouseButton::Right`] so the engine's arena derives a
/// context-menu gesture from the resulting right-button pointer up.
fn normalize_mouse_button(
    dom_button: u16,
    ctrl_key: bool,
) -> tur_engine::core::layout::MouseButton {
    let button = tur_engine::core::layout::MouseButton::from_dom(dom_button);
    if ctrl_key && button == tur_engine::core::layout::MouseButton::Left {
        tur_engine::core::layout::MouseButton::Right
    } else {
        button
    }
}

impl WasmApp {
    /// Build a DOM-wired wasm tur app instance from a [`WasmRuntime`]: create
    /// the canvas (+ wrapper / hidden textarea), wire all DOM event listeners,
    /// spawn an isolated instance via `runtime.app_builder().build(renderer, …)`,
    /// register the focus-change handler, and start the autonomous rAF loop.
    /// Resolves to the owning handle.
    pub async fn create(runtime: &WasmRuntime, cfg: WasmAppConfig) -> Result<Self, JsValue> {
        let WasmAppConfig {
            container_id,
            worker_pool,
        } = cfg;
        // Resolve the worker pool: the app's explicit choice, or the
        // runtime's built-in effectively-uncapped default.
        let worker_pool = worker_pool.unwrap_or_else(|| runtime.default_worker_pool.clone());
        let state: Rc<RefCell<Option<WasmState>>> = Rc::new(RefCell::new(None));
        let state_clone = state.clone();

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("no document"))?;

        let canvas = document
            .create_element("canvas")
            .err_to_jsval()?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .err_to_jsval()?;

        let container: web_sys::HtmlElement = if let Some(ref id) = container_id {
            document
                .get_element_by_id(id)
                .ok_or_else(|| JsValue::from_str(&format!("element #{id} not found")))?
                .dyn_into()
                .err_to_jsval()?
        } else {
            document
                .body()
                .ok_or_else(|| JsValue::from_str("no body"))?
        };

        if container_id.is_some() {
            container.append_child(&canvas).err_to_jsval()?;
            canvas
                .style()
                .set_property("width", "100%")
                .err_to_jsval()?;
            canvas
                .style()
                .set_property("height", "100%")
                .err_to_jsval()?;
            canvas
                .style()
                .set_property("display", "block")
                .err_to_jsval()?;
        } else {
            // Full-viewport mode: wrap the canvas in a div that owns the
            // safe, visible viewport. The wrapper is sized in dynamic
            // viewport units (`100dvw`/`100dvh` — fall back to `vw`/`vh`
            // where unsupported) so it shrinks with the mobile browser's
            // sliding URL bar, and its `env(safe-area-inset-*)` padding
            // (`box-sizing: border-box`) insets the content box past the
            // device notch / home indicator. The canvas fills the content
            // box, so the engine viewport == the non-occluded render area
            // — safe-area stays transparent to the app.
            let wrapper = document
                .create_element("div")
                .err_to_jsval()?
                .dyn_into::<web_sys::HtmlElement>()
                .err_to_jsval()?;
            let ws = wrapper.style();
            ws.set_property("position", "fixed").err_to_jsval()?;
            ws.set_property("top", "0").err_to_jsval()?;
            ws.set_property("left", "0").err_to_jsval()?;
            ws.set_property("width", "100vw").err_to_jsval()?;
            ws.set_property("width", "100dvw").err_to_jsval()?;
            ws.set_property("height", "100vh").err_to_jsval()?;
            ws.set_property("height", "100dvh").err_to_jsval()?;
            ws.set_property("box-sizing", "border-box").err_to_jsval()?;
            ws.set_property("padding-top", "env(safe-area-inset-top)")
                .err_to_jsval()?;
            ws.set_property("padding-right", "env(safe-area-inset-right)")
                .err_to_jsval()?;
            ws.set_property("padding-bottom", "env(safe-area-inset-bottom)")
                .err_to_jsval()?;
            ws.set_property("padding-left", "env(safe-area-inset-left)")
                .err_to_jsval()?;
            // Match the app background (#fbfcfd = tokens.bg.app) so the
            // safe-area strip is seamless with the rendered canvas.
            ws.set_property("background", "#fbfcfd").err_to_jsval()?;
            canvas
                .style()
                .set_property("width", "100%")
                .err_to_jsval()?;
            canvas
                .style()
                .set_property("height", "100%")
                .err_to_jsval()?;
            canvas
                .style()
                .set_property("display", "block")
                .err_to_jsval()?;
            let body = document
                .body()
                .ok_or_else(|| JsValue::from_str("no body"))?;
            body.style().set_property("margin", "0").err_to_jsval()?;
            body.style()
                .set_property("overflow", "hidden")
                .err_to_jsval()?;
            body.append_child(&wrapper).err_to_jsval()?;
            wrapper.append_child(&canvas).err_to_jsval()?;
        }

        // Claim touch gestures for the app. With `touch-action: none` the
        // browser will not pan/zoom the page on touch-drag (we translate
        // touchmove → ShellEvent::Wheel below). Taps are handled
        // entirely in-engine: the gesture arena synthesizes the click
        // (see `TouchUpOutcome::Tap`), and soft-keyboard focus flows from
        // the engine's focus manager to the hidden textarea via the
        // after-frame hook.
        canvas
            .style()
            .set_property("touch-action", "none")
            .err_to_jsval()?;

        let textarea = document
            .create_element("textarea")
            .err_to_jsval()?
            .dyn_into::<web_sys::HtmlTextAreaElement>()
            .err_to_jsval()?;

        textarea
            .style()
            .set_property("position", "absolute")
            .err_to_jsval()?;
        textarea
            .style()
            .set_property("opacity", "0")
            .err_to_jsval()?;
        textarea
            .style()
            .set_property("width", "1px")
            .err_to_jsval()?;
        textarea
            .style()
            .set_property("height", "1px")
            .err_to_jsval()?;
        textarea
            .style()
            .set_property("overflow", "hidden")
            .err_to_jsval()?;
        textarea
            .style()
            .set_property("border", "none")
            .err_to_jsval()?;
        textarea
            .style()
            .set_property("padding", "0")
            .err_to_jsval()?;
        textarea
            .style()
            .set_property("outline", "none")
            .err_to_jsval()?;
        textarea
            .set_attribute("autocomplete", "off")
            .err_to_jsval()?;
        textarea
            .set_attribute("autocorrect", "off")
            .err_to_jsval()?;
        textarea
            .set_attribute("autocapitalize", "off")
            .err_to_jsval()?;
        textarea
            .set_attribute("spellcheck", "false")
            .err_to_jsval()?;
        document
            .body()
            .ok_or_else(|| JsValue::from_str("no body"))?
            .append_child(&textarea)
            .err_to_jsval()?;

        // Measure the canvas's own rendered rect (CSS px). For full-viewport
        // mode this equals the wrapper's content box = the safe, visible
        // area (dvh minus safe-area padding); for container mode it's the
        // container's box. Either way the canvas — not `window.innerWidth` —
        // is the authoritative viewport.
        let (logical_width, logical_height) = if container_id.is_some() {
            let rect = container.get_bounding_client_rect();
            (rect.width() as u32, rect.height() as u32)
        } else {
            let rect = canvas.get_bounding_client_rect();
            (rect.width() as u32, rect.height() as u32)
        };
        let dpr = window.device_pixel_ratio();

        let physical_width = (logical_width as f64 * dpr) as u32;
        let physical_height = (logical_height as f64 * dpr) as u32;
        canvas.set_width(physical_width);
        canvas.set_height(physical_height);

        let renderer = WebGlVelloRenderer::new(canvas.clone(), logical_width, logical_height, dpr);

        // Spawn an isolated engine instance. The engine runs on a worker
        // thread; `HostBackend` owns the WebGL renderer on main and drives
        // it directly (render batches, image uploads, resize-on-event) —
        // `build` pushes the initial Resize
        // internally.
        // Build a WasmShell that handles cursor + text-input egress
        // AND carries the window's frame clock (rAF). The shell is
        // created before the app so it can be passed at construction
        // time — the worker's first pump already ships an initial
        // TextInputState, so a shell installed after build() could miss
        // it; the engine likewise takes the vsync source once, here.
        let state_weak: Weak<RefCell<Option<WasmState>>> = Rc::downgrade(&state_clone);
        let wasm_shell = WasmShell {
            canvas: canvas.clone(),
            state_weak: state_weak.clone(),
            vsync: Some(crate::scheduler::WasmVsyncSource::new()),
        };
        let (app, looper) = runtime
            .runtime
            .app_builder()
            .worker_pool(worker_pool)
            .renderer(
                Box::new(renderer),
                (logical_width as f64, logical_height as f64),
                dpr,
            )
            .shell(Box::new(wasm_shell))
            .build()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let resize_state = state_clone.clone();
        let resize_container_id = container_id.clone();
        let resize_closure = Closure::<dyn Fn()>::new(move || {
            let guard = resize_state.borrow();
            if let Some(s) = guard.as_ref() {
                let window = web_sys::window().unwrap();
                let dpr = window.device_pixel_ratio();
                let (logical_width, logical_height) = if resize_container_id.is_some() {
                    let document = window.document().unwrap();
                    if let Some(el) = resize_container_id
                        .as_ref()
                        .and_then(|id| document.get_element_by_id(id))
                    {
                        let rect = el.get_bounding_client_rect();
                        (rect.width() as u32, rect.height() as u32)
                    } else {
                        return;
                    }
                } else {
                    // Full-viewport mode: measure the canvas (== wrapper
                    // content box = safe area), not `window.innerHeight`.
                    let rect = s._canvas.get_bounding_client_rect();
                    (rect.width() as u32, rect.height() as u32)
                };
                let physical_width = (logical_width as f64 * dpr) as u32;
                let physical_height = (logical_height as f64 * dpr) as u32;
                s._canvas.set_width(physical_width);
                s._canvas.set_height(physical_height);
                // Resize the host-side renderer directly + forward the
                // resize to the worker for layout (single call — see
                // `TurApp::resize`).
                s.app.resize(logical_width, logical_height, dpr);
            }
        });

        // Observe the canvas directly. `ResizeObserver` fires whenever the
        // canvas border-box changes — covering window resize, orientation
        // change, *and* the mobile URL-bar slide (the wrapper's `dvh` height
        // changes → canvas resizes). `window` "resize" alone does not fire
        // on the URL-bar slide, so it is not used. The callback ignores its
        // entries argument and re-measures the canvas rect.
        let resize_observer =
            web_sys::ResizeObserver::new(resize_closure.as_ref().unchecked_ref()).err_to_jsval()?;
        resize_observer.observe(canvas.as_ref());

        let pointer_down_state = state_clone.clone();
        let pointer_down_closure =
            Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                let guard = pointer_down_state.borrow();
                if let Some(s) = guard.as_ref() {
                    let rect = s._canvas.get_bounding_client_rect();
                    let x = event.client_x() as f64 - rect.left();
                    let y = event.client_y() as f64 - rect.top();
                    // Normalize macOS Ctrl+click (a primary-button press with
                    // Ctrl held) to a secondary/right button so the engine's
                    // gesture arena derives a context-menu from it. Other
                    // platforms don't send Ctrl+click for context-menu, so
                    // this only affects the macOS convention.
                    let button = normalize_mouse_button(event.button() as u16, event.ctrl_key());
                    // DOM `MouseEvent.timeStamp` is ms since epoch — used by
                    // the engine's gesture composer for multi-click
                    // (double/triple) classification.
                    let time_ms = event.time_stamp() as u64;
                    s.app
                        .push_platform_event(ShellEvent::Pointer(PointerInput::PointerDown {
                            position: Offset::new(x, y),
                            button,
                            time_ms,
                            device: tur_engine::core::platform::PointerDeviceKind::Mouse,
                        }));
                }
            });

        canvas
            .add_event_listener_with_callback(
                "mousedown",
                pointer_down_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        let pointer_up_state = state_clone.clone();
        let pointer_up_closure =
            Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                let guard = pointer_up_state.borrow();
                if let Some(s) = guard.as_ref() {
                    let rect = s._canvas.get_bounding_client_rect();
                    let x = event.client_x() as f64 - rect.left();
                    let y = event.client_y() as f64 - rect.top();
                    let button = normalize_mouse_button(event.button() as u16, event.ctrl_key());
                    let time_ms = event.time_stamp() as u64;
                    s.app
                        .push_platform_event(ShellEvent::Pointer(PointerInput::PointerUp {
                            position: Offset::new(x, y),
                            button,
                            device: tur_engine::core::platform::PointerDeviceKind::Mouse,
                            time_ms,
                        }));
                }
            });

        canvas
            .add_event_listener_with_callback(
                "mouseup",
                pointer_up_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        let pointer_move_state = state_clone.clone();
        let pointer_move_closure =
            Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                let guard = pointer_move_state.borrow();
                if let Some(s) = guard.as_ref() {
                    let rect = s._canvas.get_bounding_client_rect();
                    let x = event.client_x() as f64 - rect.left();
                    let y = event.client_y() as f64 - rect.top();
                    let time_ms = event.time_stamp() as u64;
                    s.app
                        .push_platform_event(ShellEvent::Pointer(PointerInput::PointerMove {
                            position: Offset::new(x, y),
                            device: tur_engine::core::platform::PointerDeviceKind::Mouse,
                            time_ms,
                        }));
                }
            });

        canvas
            .add_event_listener_with_callback(
                "mousemove",
                pointer_move_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        let wheel_state = state_clone.clone();
        let wheel_closure =
            Closure::<dyn Fn(web_sys::WheelEvent)>::new(move |event: web_sys::WheelEvent| {
                event.prevent_default();
                let guard = wheel_state.borrow();
                if let Some(s) = guard.as_ref() {
                    let rect = s._canvas.get_bounding_client_rect();
                    let x = event.client_x() as f64 - rect.left();
                    let y = event.client_y() as f64 - rect.top();
                    s.app.push_platform_event(ShellEvent::Wheel {
                        delta_x: event.delta_x(),
                        delta_y: event.delta_y(),
                        position: Offset::new(x, y),
                    });
                }
            });

        canvas
            .add_event_listener_with_callback("wheel", wheel_closure.as_ref().unchecked_ref())
            .err_to_jsval()?;

        // Touch handling for mobile. Touch events are dispatched as
        // native touch pointer events (`device: Touch`) into the
        // engine's gesture arena, which resolves drag-vs-scroll
        // competition using a slop threshold (18px, matching Flutter's
        // `kTouchSlop`) and — when no drag/scroll wins — synthesizes
        // the tap→click itself (host-agnostic; same behavior on browser,
        // Android, and desktop).
        //
        // - **touchstart**: push `PointerDown { device: Touch }`. The
        //   arena collects candidates from the hit-path but does NOT
        //   dispatch to elements yet. Not preventDefaulted.
        //
        // - **touchmove**: `preventDefault` (stops browser panning AND
        //   mouse-event synthesis for the entire touch sequence). Push
        //   `PointerMove { device: Touch }`. The arena checks slop:
        //   if movement < 18px → no dispatch (still ambiguous); if ≥18px
        //   → resolve to drag winner (dispatch PointerDown+PointerMove)
        //   or scroll winner (dispatch Wheel).
        //
        // - **touchend**: `preventDefault` (suppresses the browser's
        //   synthesized `mousedown`/`mouseup`/`click` so the tap doesn't
        //   double-fire — the engine synthesizes it). Push
        //   `PointerUp { device: Touch }`. If the arena resolved a drag
        //   → dispatch PointerUp to release capture. Otherwise the arena
        //   classifies the gesture as a tap (short + sub-slop) and the
        //   handler synthesizes a mouse down→up to drive click/focus, or
        //   idle (too long / too far).
        //
        // - **touchcancel**: push `PointerCancel { device: Touch }`.
        //   The arena releases any captured drag without firing a click.
        //
        // Soft-keyboard / caret focus does NOT depend on the browser's
        // synthesized clicks: the engine's focus manager (driven by the
        // engine-synthesized click) triggers a text-input state push,
        // which the shell (registered at build time) consumes to
        // call `textarea.focus()` + position the caret.
        let touch_start_state = state_clone.clone();
        let touch_start_closure =
            Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                let Some(t) = event.touches().get(0) else {
                    return;
                };
                let guard = touch_start_state.borrow();
                let Some(s) = guard.as_ref() else {
                    return;
                };
                let rect = s._canvas.get_bounding_client_rect();
                let x = t.client_x() as f64 - rect.left();
                let y = t.client_y() as f64 - rect.top();
                let time_ms = event.time_stamp() as u64;
                s.app
                    .push_platform_event(ShellEvent::Pointer(PointerInput::PointerDown {
                        position: Offset::new(x, y),
                        button: tur_engine::core::layout::MouseButton::Left,
                        time_ms,
                        device: tur_engine::core::platform::PointerDeviceKind::Touch,
                    }));
            });

        canvas
            .add_event_listener_with_callback(
                "touchstart",
                touch_start_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        let touch_move_state = state_clone.clone();
        let touch_move_closure =
            Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                let Some(t) = event.touches().get(0) else {
                    return;
                };
                event.prevent_default();
                let guard = touch_move_state.borrow();
                let Some(s) = guard.as_ref() else {
                    return;
                };
                let rect = s._canvas.get_bounding_client_rect();
                let x = t.client_x() as f64 - rect.left();
                let y = t.client_y() as f64 - rect.top();
                let time_ms = event.time_stamp() as u64;
                s.app
                    .push_platform_event(ShellEvent::Pointer(PointerInput::PointerMove {
                        position: Offset::new(x, y),
                        device: tur_engine::core::platform::PointerDeviceKind::Touch,
                        time_ms,
                    }));
            });

        canvas
            .add_event_listener_with_callback(
                "touchmove",
                touch_move_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        let touch_end_state = state_clone.clone();
        let touch_end_closure =
            Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                // `preventDefault` so the browser does NOT synthesize the
                // trailing `mousedown`/`mouseup`/`click` for this tap. The
                // engine synthesizes the click itself (gesture arena
                // `TouchUpOutcome::Tap`), and without this suppression a
                // pure tap would fire twice (engine-synthesized click +
                // browser-synthesized mouse path). Soft-keyboard focus is
                // unaffected: it flows from the engine's focus manager
                // (driven by the engine-synthesized click) to the hidden
                // textarea via the after-frame hook, not via browser
                // `click` events. (Suppressing on `touchend` — rather than
                // `touchstart` — is the minimal change and matches what
                // `touchmove` already does.)
                event.prevent_default();
                let Some(t) = event.changed_touches().get(0) else {
                    let guard = touch_end_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        let time_ms = event.time_stamp() as u64;
                        s.app
                            .push_platform_event(ShellEvent::Pointer(PointerInput::PointerUp {
                                position: Offset::new(0.0, 0.0),
                                button: tur_engine::core::layout::MouseButton::Left,
                                device: tur_engine::core::platform::PointerDeviceKind::Touch,
                                time_ms,
                            }));
                    }
                    return;
                };
                let guard = touch_end_state.borrow();
                let Some(s) = guard.as_ref() else {
                    return;
                };
                let rect = s._canvas.get_bounding_client_rect();
                let x = t.client_x() as f64 - rect.left();
                let y = t.client_y() as f64 - rect.top();
                let time_ms = event.time_stamp() as u64;
                s.app
                    .push_platform_event(ShellEvent::Pointer(PointerInput::PointerUp {
                        position: Offset::new(x, y),
                        button: tur_engine::core::layout::MouseButton::Left,
                        device: tur_engine::core::platform::PointerDeviceKind::Touch,
                        time_ms,
                    }));
            });

        canvas
            .add_event_listener_with_callback(
                "touchend",
                touch_end_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        let touch_cancel_state = state_clone.clone();
        let touch_cancel_closure =
            Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |_event: web_sys::TouchEvent| {
                let guard = touch_cancel_state.borrow();
                if let Some(s) = guard.as_ref() {
                    s.app
                        .push_platform_event(ShellEvent::Pointer(PointerInput::PointerCancel {
                            device: tur_engine::core::platform::PointerDeviceKind::Touch,
                        }));
                }
            });

        canvas
            .add_event_listener_with_callback(
                "touchcancel",
                touch_cancel_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        // Context menu listener. We only `preventDefault` to suppress the
        // native browser menu. The context-menu *gesture* itself is derived
        // inside the engine from the right-button `PointerUp` (the mouseup
        // listener above already pushes that — including macOS Ctrl+click,
        // which is normalized to a Right button). We must NOT push a
        // separate event here, otherwise a physical right-click would fire
        // context-menu twice (once from mouseup, once from here).
        let context_state = state_clone.clone();
        let context_closure =
            Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                let _ = context_state;
                event.prevent_default();
            });

        canvas
            .add_event_listener_with_callback(
                "contextmenu",
                context_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        canvas.set_attribute("tabindex", "0").err_to_jsval()?;
        canvas
            .style()
            .set_property("outline", "none")
            .err_to_jsval()?;

        let keydown_state = state_clone.clone();
        let keydown_closure =
            Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
                let guard = keydown_state.borrow();
                if let Some(s) = guard.as_ref() {
                    if s.is_composing.get() {
                        return;
                    }
                    s.app.push_platform_event(ShellEvent::Key(KeyEvent {
                        key: event.key(),
                        code: event.code(),
                        modifiers: Modifiers {
                            ctrl: event.ctrl_key(),
                            shift: event.shift_key(),
                            alt: event.alt_key(),
                            meta: event.meta_key(),
                        },
                        event_type: KeyEventType::Down,
                    }));
                }
            });

        canvas
            .add_event_listener_with_callback("keydown", keydown_closure.as_ref().unchecked_ref())
            .err_to_jsval()?;

        textarea
            .add_event_listener_with_callback("keydown", keydown_closure.as_ref().unchecked_ref())
            .err_to_jsval()?;

        let keyup_state = state_clone.clone();
        let keyup_closure =
            Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
                let guard = keyup_state.borrow();
                if let Some(s) = guard.as_ref() {
                    if s.is_composing.get() {
                        return;
                    }
                    s.app.push_platform_event(ShellEvent::Key(KeyEvent {
                        key: event.key(),
                        code: event.code(),
                        modifiers: Modifiers {
                            ctrl: event.ctrl_key(),
                            shift: event.shift_key(),
                            alt: event.alt_key(),
                            meta: event.meta_key(),
                        },
                        event_type: KeyEventType::Up,
                    }));
                }
            });

        canvas
            .add_event_listener_with_callback("keyup", keyup_closure.as_ref().unchecked_ref())
            .err_to_jsval()?;

        textarea
            .add_event_listener_with_callback("keyup", keyup_closure.as_ref().unchecked_ref())
            .err_to_jsval()?;

        let comp_start_state = state_clone.clone();
        let compositionstart_closure = Closure::<dyn Fn(web_sys::CompositionEvent)>::new(
            move |_event: web_sys::CompositionEvent| {
                let guard = comp_start_state.borrow();
                if let Some(s) = guard.as_ref() {
                    s.is_composing.set(true);
                    s.app
                        .push_platform_event(ShellEvent::Ime(ImeEvent::CompositionStart));
                }
            },
        );

        textarea
            .add_event_listener_with_callback(
                "compositionstart",
                compositionstart_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        let comp_update_state = state_clone.clone();
        let compositionupdate_closure = Closure::<dyn Fn(web_sys::CompositionEvent)>::new(
            move |event: web_sys::CompositionEvent| {
                let guard = comp_update_state.borrow();
                if let Some(s) = guard.as_ref() {
                    let text = event.data().unwrap_or_default();
                    s.app
                        .push_platform_event(ShellEvent::Ime(ImeEvent::CompositionUpdate {
                            text,
                            cursor: None,
                        }));
                }
            },
        );

        textarea
            .add_event_listener_with_callback(
                "compositionupdate",
                compositionupdate_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        let comp_end_state = state_clone.clone();
        let compositionend_closure = Closure::<dyn Fn(web_sys::CompositionEvent)>::new(
            move |event: web_sys::CompositionEvent| {
                let guard = comp_end_state.borrow();
                if let Some(s) = guard.as_ref() {
                    s.is_composing.set(false);
                    let text = event.data().unwrap_or_default();
                    s.app
                        .push_platform_event(ShellEvent::Ime(ImeEvent::CompositionEnd { text }));
                    s.textarea.set_value("");
                }
            },
        );

        textarea
            .add_event_listener_with_callback(
                "compositionend",
                compositionend_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;

        // Paste listener — when the user presses Cmd+V (or Ctrl+V) while
        // the hidden textarea is focused, the browser fires a `paste`
        // event with `clipboardData`. We forward the text to the engine
        // as a ClipboardPlatformPasteEvent (PlatformEvent::Custom);
        // tur-clipboard's ClipboardPlatformSubsystem re-emits it as a
        // ClipboardPasteEvent (AppEvent::Custom), which tur-text's
        // ClipboardPasteSubsystem consumes to insert into the focused
        // editable.
        let paste_state = state_clone.clone();
        let paste_closure = Closure::<dyn Fn(web_sys::ClipboardEvent)>::new(
            move |event: web_sys::ClipboardEvent| {
                event.prevent_default();
                let text = event
                    .clipboard_data()
                    .and_then(|d| d.get_data("text/plain").ok())
                    .unwrap_or_default();
                if text.is_empty() {
                    return;
                }
                let guard = paste_state.borrow();
                if let Some(s) = guard.as_ref() {
                    s.app.push_platform_event(tur_engine::platform_paste(text));
                }
            },
        );

        textarea
            .add_event_listener_with_callback("paste", paste_closure.as_ref().unchecked_ref())
            .err_to_jsval()?;

        let wasm_state = WasmState {
            app,
            _canvas: canvas,
            textarea,
            is_composing: Cell::new(false),
            _resize_closure: resize_closure,
            _resize_observer: resize_observer,
            _pointer_down_closure: pointer_down_closure,
            _pointer_up_closure: pointer_up_closure,
            _pointer_move_closure: pointer_move_closure,
            _wheel_closure: wheel_closure,
            _context_closure: context_closure,
            _touch_start_closure: touch_start_closure,
            _touch_move_closure: touch_move_closure,
            _touch_end_closure: touch_end_closure,
            _touch_cancel_closure: touch_cancel_closure,
            _keydown_closure: keydown_closure,
            _keyup_closure: keyup_closure,
            _compositionstart_closure: compositionstart_closure,
            _compositionupdate_closure: compositionupdate_closure,
            _compositionend_closure: compositionend_closure,
            _paste_closure: paste_closure,
        };

        *state_clone.borrow_mut() = Some(wasm_state);

        // Autonomous loop. The engine owns the frame logic (clock advance
        // is its own `StdClock`, no manual forwarding); this driver just
        // arms rAF / setTimeout per the engine's `NextFrame` verdict.
        // DOM side-effects that depend on focus state (textarea focus /
        // caret positioning) live in the `focus_changed_handler`
        // registered below, which fires exactly when the worker ships a
        // deduped text-input state (on editable↔non-editable
        // transitions *and* caret moves).
        //
        // Async pump: the wake trampoline the engine installs hands a
        // `Box::pin(async { wake().await })` future to the spawn closure
        // we pass here. We use `wasm_bindgen_futures::spawn_local`, which
        // runs the future cooperatively on the JS event loop — the wasm
        // main thread never blocks (no `Atomics.wait`).
        // Spawn the autonomous loop. The embedder (wasm main thread)
        // drives the future via `wasm_bindgen_futures::spawn_local`. The
        // loop bootstraps automatically: `app_builder().build(...)` pushed
        // an initial resize → worker pumps → FrameOutcome arrives → main requests
        // the next vsync. No manual kick needed.
        wasm_bindgen_futures::spawn_local(looper.run());

        Ok(WasmApp { state: state_clone })
    }

    /// Evaluate `js_source` as an ES module (supports real
    /// `import { ... } from "tur:..."`, resolved by the engine's module
    /// loader), then start the frame loop. Used by the website to load the
    /// playground-view bundle. The module must export `start()` (the
    /// module lifecycle contract).
    pub async fn load_and_run_module(&self, js_source: &str) -> Result<(), JsValue> {
        let app = {
            let guard = self.state.borrow();
            let Some(s) = guard.as_ref() else {
                return Err(JsValue::from_str("app not initialized"));
            };
            s.app.clone()
        };
        app.backend()
            .load_module(js_source)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        // The worker self-paints on load (dirty state → coalesced self-wake);
        // no embedder paint request needed.
        Ok(())
    }

    /// JSON snapshot of the root node, or `""` if no tree is mounted.
    /// Shape: `{ id, name, label, props, layout:{relative,absolute,width,height,extra?}, queryKey?, children:[{id}, ...] }`.
    ///
    /// Async: the underlying RPC is `async`. Drives it to completion via
    /// `wasm_bindgen_futures::future_to_promise` — the JS caller `await`s
    /// the returned `Promise`.
    pub fn element_tree(&self) -> js_sys::Promise {
        // Bail out synchronously if no state is mounted (avoids borrowing
        // the RefCell across the async boundary).
        let app = {
            let guard = self.state.borrow();
            match guard.as_ref() {
                Some(s) => s.app.clone(),
                None => return js_sys::Promise::resolve(&JsValue::from_str("")),
            }
        };
        wasm_bindgen_futures::future_to_promise(async move {
            let s = app
                .backend()
                .eval_js("JSON.stringify(turDevTool.elementTree())")
                .await;
            Ok(JsValue::from_str(&s))
        })
    }

    /// JSON snapshot of a single node by id (full subtree metadata; children
    /// are returned as bare `{id}` handles). Returns `""` if not found.
    pub fn get_element(&self, id: u32) -> js_sys::Promise {
        let app = {
            let guard = self.state.borrow();
            match guard.as_ref() {
                Some(s) => s.app.clone(),
                None => return js_sys::Promise::resolve(&JsValue::from_str("")),
            }
        };
        let source = format!("JSON.stringify(turDevTool.getElement({id}))");
        wasm_bindgen_futures::future_to_promise(async move {
            let s = app.backend().eval_js(&source).await;
            Ok(JsValue::from_str(&s))
        })
    }
}
