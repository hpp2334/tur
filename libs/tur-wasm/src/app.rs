use crate::fonts::WasmFontLoader;
use boa_engine::Source;
use boa_engine::context::time::{Clock, JsInstant};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use tur_clipboard_wasm::{Clipboard, TurClipboardPlugin, WasmClipboard};
use tur_engine::core::app::NextFrame;
use tur_engine::core::layout::Offset;
use tur_engine::core::platform::key_event::{KeyEvent, KeyEventType, Modifiers};
use tur_engine::core::platform::{ImeEvent, PlatformEvent, PointerInput};
use tur_engine::renderer::vello::WebGlVelloRenderer;
use tur_engine::{LoopDriver, TurApp};
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

/// Embedder-side `CursorBackend`: the engine pushes the resolved cursor here
/// during the frame loop, and we apply it to the host canvas.
struct WasmCursor {
    canvas: web_sys::HtmlCanvasElement,
}

impl tur_engine::CursorBackend for WasmCursor {
    fn set_cursor(&mut self, cursor: tur_engine::core::platform::Cursor) {
        let _ = self.canvas.style().set_property("cursor", cursor.as_str());
    }
}

/// After-frame callback run inside the engine's after-frame hook, where a
/// `&mut boa `Context`` is available (so a JS-evaluating callback can drain
/// pending host resolutions). The generic textarea / caret-focus logic always
/// runs after this.
pub type AfterFrameHook = Rc<dyn Fn(&mut boa_engine::Context)>;

/// Configuration for building a shared wasm tur runtime via
/// [`WasmRuntime::create`].
///
/// `tur-wasm` is a reusable embedder lib (no playground / demo-plugin code):
/// the host cdylib supplies the engine-customization callback (extra plugins)
/// while `tur-wasm` owns all the generic capability backends.
pub struct WasmRuntimeConfig {
    /// Customize the [`tur_engine::TurRuntimeBuilder`] before `build()` — the
    /// caller adds its own plugins (and may override the default capabilities).
    /// `tur-wasm` has already registered the standard plugin set + clipboard /
    /// http / filepicker / cursor backends before invoking this.
    pub configure: Box<dyn FnOnce(tur_engine::TurRuntimeBuilder) -> tur_engine::TurRuntimeBuilder>,
}

/// The shared wasm runtime — created once via [`WasmRuntime::create`]. Owns the
/// [`tur_engine::TurRuntime`] (fonts, clock, capabilities, plugins). Spawn
/// isolated instances (each with its own canvas/DOM or headless) via
/// [`WasmRuntime::create_app`] / [`WasmRuntime::create_headless_app`].
pub struct WasmRuntime {
    runtime: Rc<tur_engine::TurRuntime>,
}

impl WasmRuntime {
    /// Build the shared runtime with the wasm-default capabilities (WasmClock,
    /// WasmFontLoader, WasmClipboard, WasmHttp, WasmFilePicker) + the standard
    /// plugin set, then apply the embedder's `configure` callback (extra
    /// plugins / capability overrides). No canvas/DOM — instances are spawned
    /// separately.
    pub fn create(cfg: WasmRuntimeConfig) -> Result<Self, JsValue> {
        // Initialize the wasm-bindgen-rayon thread pool. Must be called
        // before any `spawn_blocking`-style work; requires
        // SharedArrayBuffer (COOP/COEP headers — already configured in
        // the dev server). The returned `Promise` resolves once the
        // workers are ready; we don't await it here (the pool lazily
        // spins up workers as needed).
        //
        // Note: the wasm embedder currently uses inline mode
        // (`runtime.create_app`) because `WebGlVelloRenderer` holds web-sys
        // types (`!Send` across web-worker realms). True threaded wasm
        // rendering requires splitting the renderer (main) from the JS
        // engine (worker) — a future architectural change. This call
        // initializes the pool so rayon-backed crates (e.g. usvg image
        // decoding) can offload to workers in the meantime.
        let _ = wasm_bindgen_rayon::init_thread_pool(4);

        let builder = tur_engine::TurRuntime::builder()
            .font_loader(std::sync::Arc::new(WasmFontLoader::new()))
            .clock(std::sync::Arc::new(WasmClock))
            .capability(Clipboard::new(WasmClipboard))
            .capability(Http::new(WasmHttp))
            .capability(FilePicker::new(WasmFilePicker))
            .plugin(tur_engine::TurStdPlugin)
            .plugin(tur_animation::TurAnimationPlugin)
            .plugin(TurClipboardPlugin)
            .plugin(TurNetPlugin)
            .plugin(TurFilePickerPlugin);
        // Let the embedder add its own plugins / override capabilities.
        let runtime = (cfg.configure)(builder)
            .build()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { runtime })
    }

    /// Access the underlying [`tur_engine::TurRuntime`] (for spawning raw
    /// instances outside the wasm DOM-wired helpers).
    pub fn runtime(&self) -> &Rc<tur_engine::TurRuntime> {
        &self.runtime
    }
}

/// Configuration for building a DOM-wired wasm app instance via
/// [`WasmApp::create`].
pub struct WasmAppConfig {
    /// `None` ⇒ full-viewport canvas (own wrapper `div`); `Some(id)` ⇒ embed
    /// the canvas inside the element with that id.
    pub container_id: Option<String>,
    /// Extra after-frame work run inside the engine's after-frame hook (where
    /// a `&mut boa Context` is available). `None` for embedders with no such
    /// work.
    pub after_frame: Option<AfterFrameHook>,
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
    /// spawn an isolated instance via `runtime.create_app(renderer, …)`,
    /// register the after-frame hook, and start the autonomous rAF loop.
    /// Resolves to the owning handle.
    pub async fn create(runtime: &WasmRuntime, cfg: WasmAppConfig) -> Result<Self, JsValue> {
        let WasmAppConfig {
            container_id,
            after_frame: after_frame_hook,
        } = cfg;
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
        // touchmove → PlatformEvent::Wheel below). Taps are handled
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

        // Spawn an isolated instance from the shared runtime, attached to this
        // canvas's renderer. `create_app` pushes the initial Resize internally.
        let app = runtime
            .runtime
            .create_app(
                Box::new(renderer),
                (logical_width as f64, logical_height as f64),
                dpr,
            )
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // The cursor backend is per-instance (it targets this canvas's DOM
        // element), so it can't be a shared runtime capability. Override the
        // shell's cursor backend now that the instance exists.
        app.set_cursor_backend(Rc::new(RefCell::new(WasmCursor {
            canvas: canvas.clone(),
        })));

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
                s.app.push_platform_event(PlatformEvent::Resize {
                    logical_width,
                    logical_height,
                    dpr,
                });
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
                        .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerDown {
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
                        .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerUp {
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
                        .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerMove {
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
                    s.app.push_platform_event(PlatformEvent::Wheel {
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
        // engine-synthesized click) exposes `focused_is_editable()`,
        // which the after-frame hook reads to call `textarea.focus()`.
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
                    .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerDown {
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
                    .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerMove {
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
                        s.app.push_platform_event(PlatformEvent::Pointer(
                            PointerInput::PointerUp {
                                position: Offset::new(0.0, 0.0),
                                button: tur_engine::core::layout::MouseButton::Left,
                                device: tur_engine::core::platform::PointerDeviceKind::Touch,
                                time_ms,
                            },
                        ));
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
                    .push_platform_event(PlatformEvent::Pointer(PointerInput::PointerUp {
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
                    s.app.push_platform_event(PlatformEvent::Pointer(
                        PointerInput::PointerCancel {
                            device: tur_engine::core::platform::PointerDeviceKind::Touch,
                        },
                    ));
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
                    s.app.push_platform_event(PlatformEvent::Key(KeyEvent {
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
                    s.app.push_platform_event(PlatformEvent::Key(KeyEvent {
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
                        .push_platform_event(PlatformEvent::Ime(ImeEvent::CompositionStart));
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
        let compositionupdate_closure =
            Closure::<dyn Fn(web_sys::CompositionEvent)>::new(
                move |event: web_sys::CompositionEvent| {
                    let guard = comp_update_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        let text = event.data().unwrap_or_default();
                        s.app.push_platform_event(PlatformEvent::Ime(
                            ImeEvent::CompositionUpdate { text, cursor: None },
                        ));
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
                        .push_platform_event(PlatformEvent::Ime(ImeEvent::CompositionEnd { text }));
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
        // arms rAF / setTimeout per the engine's `NextFrame` verdict. The
        // `after_frame` hook — fired by the engine after each wake — does
        // the DOM side-effects that used to live in `FrameLoop::on_frame`
        // (file-pick resolution, textarea focus / caret positioning). It
        // holds a `Weak` into `state` so there's no reference cycle
        // (`state` → `app` → `after_frame` → `state`).
        let app = state_clone
            .borrow()
            .as_ref()
            .expect("wasm state just set")
            .app
            .clone();
        let state_weak: Weak<RefCell<Option<WasmState>>> = Rc::downgrade(&state_clone);
        let after_frame: Rc<dyn Fn(tur_engine::core::app::FrameOutcome)> =
            Rc::new(move |_outcome| {
                let Some(state) = state_weak.upgrade() else {
                    return;
                };
                let mut guard = state.borrow_mut();
                let Some(s) = guard.as_mut() else {
                    return;
                };

                // Embedder-supplied after-frame work (run with a live
                // `&mut Context` between frames). The generic textarea /
                // caret focus logic below runs after it.
                if let Some(hook) = after_frame_hook.as_ref() {
                    let hook = hook.clone();
                    s.app.with_boa_context(move |ctx| hook(ctx));
                }

                let is_editable = s.app.focused_is_editable();
                if is_editable {
                    let _ = s.textarea.focus();
                    if let Some((x, y, _w, _h)) = s.app.focused_cursor_rect() {
                        let _ = s.textarea.style().set_property("left", &format!("{x}px"));
                        let _ = s.textarea.style().set_property("top", &format!("{y}px"));
                    }
                }
            });
        app.set_after_frame_hook(Some(after_frame));
        // `start` registers the driver and runs frame 1 (which processes
        // the resize pushed above), then arms follow-up wake-ups per the
        // engine's verdict.
        app.start(WasmLoopDriver::new());

        Ok(WasmApp { state: state_clone })
    }

    pub fn load_and_run_js(&self, js_source: &str) -> Result<(), JsValue> {
        let mut guard = self.state.borrow_mut();
        let state = guard
            .as_mut()
            .ok_or_else(|| JsValue::from_str("app not initialized"))?;
        state
            .app
            .load_js(js_source)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Request a paint; `request_paint` re-arms the autonomous loop (via
        // the driver's `request_next(Vsync)`), so the bundle renders on the
        // next frame without any manual pump.
        state.app.request_paint();

        Ok(())
    }

    /// Evaluate `js_source` as an ES module (supports real
    /// `import { ... } from "tur:..."`, resolved by the engine's module
    /// loader), then start the frame loop. Used by the website to load the
    /// playground-view bundle.
    pub fn load_and_run_module(&self, js_source: &str) -> Result<(), JsValue> {
        let mut guard = self.state.borrow_mut();
        let state = guard
            .as_mut()
            .ok_or_else(|| JsValue::from_str("app not initialized"))?;
        state
            .app
            .load_module(js_source)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        state.app.request_paint();

        Ok(())
    }

    /// JSON snapshot of the root node, or `""` if no tree is mounted.
    /// Shape: `{ id, name, label, props, layout:{relative,absolute,width,height,extra?}, queryKey?, children:[{id}, ...] }`.
    pub fn element_tree(&self) -> String {
        let mut guard = self.state.borrow_mut();
        let Some(s) = guard.as_mut() else {
            return String::new();
        };
        s.app.with_boa_context(|ctx| {
            ctx.eval(Source::from_bytes(
                "JSON.stringify(turDevTool.elementTree())",
            ))
            .ok()
            .and_then(|r| r.as_string().map(|s| s.to_std_string_escaped()))
            .unwrap_or_default()
        })
    }

    /// JSON snapshot of a single node by id (full subtree metadata; children
    /// are returned as bare `{id}` handles). Returns `""` if not found.
    pub fn get_element(&self, id: u32) -> String {
        let mut guard = self.state.borrow_mut();
        let Some(s) = guard.as_mut() else {
            return String::new();
        };
        let id_str = format!("JSON.stringify(turDevTool.getElement({id}))");
        s.app.with_boa_context(move |ctx| {
            ctx.eval(Source::from_bytes(&id_str))
                .ok()
                .and_then(|r| r.as_string().map(|s| s.to_std_string_escaped()))
                .unwrap_or_default()
        })
    }
}

/// `LoopDriver` for the wasm embedder, backed by `requestAnimationFrame`
/// (`Vsync`) and `setTimeout` (`After(d)`). The engine drives itself: each
/// autonomous frame runs in [`TurApp::wake`] (clock advance is the engine's
/// own `StdClock` — no manual time forwarding), the `after_frame` hook handles
/// DOM side-effects, and this driver just arms the next wake-up per the
/// engine's `NextFrame` verdict. When the engine reports `Idle`, the loop
/// stops entirely (zero rAF, zero renders) until [`TurApp::push_platform_event`]
/// re-arms it via `request_next(Vsync)`.
///
/// The two `Closure`s are long-lived and re-registered for every wake-up
/// (created once via `Rc::new_cyclic` with `Weak` back-refs) so a wake that
/// requests the next wake-up mid-invocation doesn't drop its own trampoline —
/// the same closure-lifetime fix the old `FrameLoop` needed.
struct WasmLoopDriver {
    /// Engine wake trampoline, set once via [`LoopDriver::set_wake`] at
    /// [`TurApp::start`].
    wake: RefCell<Option<Rc<dyn Fn()>>>,
    /// Pending rAF / setTimeout handle, if any. `None` ⇒ nothing pending
    /// (the loop is idle). Cleared by the trampoline when it fires.
    raf_id: Cell<Option<i32>>,
    timeout_id: Cell<Option<i32>>,
    raf_closure: Closure<dyn Fn()>,
    timeout_closure: Closure<dyn Fn()>,
}

impl WasmLoopDriver {
    fn new() -> Rc<Self> {
        Rc::<Self>::new_cyclic(|weak| {
            let weak_raf = weak.clone();
            let raf_closure = Closure::<dyn Fn()>::new(move || {
                if let Some(d) = weak_raf.upgrade() {
                    d.fire_raf();
                }
            });
            let weak_to = weak.clone();
            let timeout_closure = Closure::<dyn Fn()>::new(move || {
                if let Some(d) = weak_to.upgrade() {
                    d.fire_timeout();
                }
            });
            Self {
                wake: RefCell::new(None),
                raf_id: Cell::new(None),
                timeout_id: Cell::new(None),
                raf_closure,
                timeout_closure,
            }
        })
    }

    /// rAF trampoline entry: clear the handle, then fire the engine wake.
    fn fire_raf(&self) {
        self.raf_id.set(None);
        if let Some(wake) = self.wake.borrow().as_ref().cloned() {
            wake();
        }
    }

    /// setTimeout trampoline entry: clear the handle, then fire the wake
    /// (which re-arms via `request_next` for any further scheduling).
    fn fire_timeout(&self) {
        self.timeout_id.set(None);
        if let Some(wake) = self.wake.borrow().as_ref().cloned() {
            wake();
        }
    }

    /// Cancel any pending rAF / setTimeout so a fresh `request_next` starts
    /// from a clean slate (avoids double-firing when input re-arms an idle
    /// loop that had a timer outstanding).
    fn cancel_pending(&self) {
        if let Some(id) = self.raf_id.take()
            && let Some(window) = web_sys::window()
        {
            let _ = window.cancel_animation_frame(id);
        }
        if let Some(id) = self.timeout_id.take()
            && let Some(window) = web_sys::window()
        {
            window.clear_timeout_with_handle(id);
        }
    }
}

impl LoopDriver for WasmLoopDriver {
    fn set_wake(&self, wake: Rc<dyn Fn()>) {
        *self.wake.borrow_mut() = Some(wake);
    }

    fn request_next(&self, next: NextFrame) {
        self.cancel_pending();
        let Some(window) = web_sys::window() else {
            return;
        };
        match next {
            NextFrame::Idle => {}
            NextFrame::Vsync => {
                let id = window
                    .request_animation_frame(self.raf_closure.as_ref().unchecked_ref())
                    .unwrap_or(-1);
                if id >= 0 {
                    self.raf_id.set(Some(id));
                }
            }
            NextFrame::After(delay) => {
                let ms = delay.as_millis().min(i32::MAX as u128) as i32;
                let id = window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        self.timeout_closure.as_ref().unchecked_ref(),
                        ms.max(1),
                    )
                    .unwrap_or(0);
                self.timeout_id.set(Some(id));
            }
        }
    }
}
