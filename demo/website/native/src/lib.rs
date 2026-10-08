//! WebAssembly entry for the tur website.
//!
//! `tur-wasm` is a reusable embedder lib (it owns all the DOM wiring + engine
//! glue but exports no `#[wasm_bindgen]` surface and pulls in no playground
//! code). This crate is the website's *own* `.so`: it wraps `tur-wasm`'s
//! [`tur_wasm::WasmRuntime`] + [`tur_wasm::WasmApp`] builders and wires the
//! playground's rut compile service ([`tur_playground::TurRutPlaygroundPlugin`],
//! the retired swc plugin's replacement). JS imports `TurWebsiteApp` from the
//! generated `tur_website.js` and boots the playground via
//! `loadAndRunRutModule(playgroundSource())`.
//!
//! Mirrors the Android split: `tur-android` (pure rlib) vs `demo/compose/native`
//! (the app's own cdylib that adds the demo plugin set).

// Everything is wasm32-only — on a host `cargo check --workspace` this is an
// empty (but compiling) cdylib.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use tur_playground::TurRutPlaygroundPlugin;

#[cfg(target_arch = "wasm32")]
pub const PLAYGROUND_RUT: &str = include_str!("../../../../rut/playground/playground.rut");
#[cfg(target_arch = "wasm32")]
pub const CASES_GEN_RUT: &str = include_str!("../../../../rut/playground/cases_gen.rut");

/// One-time wasm init (panic hook + tracing). Called automatically on module
/// instantiation via the `#[wasm_bindgen(start)]` attribute.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_entry() {
    tur_wasm::init();
}

/// A running tur website app. Construct via [`TurWebsiteApp::create`] (full
/// viewport) or [`TurWebsiteApp::create_in`] (embedded in a container element).
/// Load a rut module (e.g. the playground, `rut/playground`) via
/// `loadAndRunRutModule`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct TurWebsiteApp {
    /// The shared runtime (fonts, clock, capabilities, plugins). Kept alive
    /// for the app's lifetime so the instance can reference it.
    _runtime: tur_wasm::WasmRuntime,
    /// The DOM-wired instance (canvas + renderer + loop).
    app: tur_wasm::WasmApp,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl TurWebsiteApp {
    /// Full-viewport canvas: the app owns the entire window.
    pub fn create() -> js_sys::Promise {
        Self::create_internal(None, None)
    }

    /// Embed the canvas inside the element with the given id.
    pub fn create_in(container_id: String) -> js_sys::Promise {
        Self::create_internal(Some(container_id), None)
    }

    /// Like [`Self::create`] / [`Self::create_in`], but sets the renderer's
    /// base (background) color. `background` is a CSS-style hex string
    /// (`#RGB`, `#RRGGBB`, or `#RRGGBBAA`); pass `None`/`null` for the
    /// default opaque white. The promise rejects on a malformed color.
    #[wasm_bindgen(js_name = createWithBackground)]
    pub fn create_with_background(
        container_id: Option<String>,
        background: Option<String>,
    ) -> js_sys::Promise {
        Self::create_internal(container_id, background)
    }

    fn create_internal(
        container_id: Option<String>,
        background: Option<String>,
    ) -> js_sys::Promise {
        wasm_bindgen_futures::future_to_promise(async move {
            // Parse the optional base color up front so a malformed value
            // rejects before any DOM/runtime work happens.
            let base_color = match background.as_deref().map(str::parse) {
                Some(Ok(color)) => Some(color),
                Some(Err(_)) => {
                    return Err(JsValue::from_str(
                        "invalid background color: expected #RGB, #RRGGBB, or #RRGGBBAA",
                    ));
                }
                None => None,
            };
            // Build the shared runtime once with the playground's rut
            // compile-service extension (the swc plugin is retired — the
            // editor compiles rut via `pg_compile`).
            let runtime = tur_wasm::WasmRuntime::create(tur_wasm::WasmRuntimeConfig {
                configure: Box::new(|b| b.plugin(TurRutPlaygroundPlugin)),
                worker_pools: Vec::new(),
            })?;
            // Spawn an isolated DOM-wired instance from it.
            let app = tur_wasm::WasmApp::create(
                &runtime,
                tur_wasm::WasmAppConfig {
                    container_id,
                    worker_pool: None,
                    base_color,
                },
            )
            .await?;
            Ok(JsValue::from(TurWebsiteApp {
                _runtime: runtime,
                app,
            }))
        })
    }

    /// Compile + boot `source` as a **rut** module and render. The zero-JS
    /// load path (the engine's `load_rut_module` RPC) — the module exports
    /// `entry fn start()`.
    #[wasm_bindgen(js_name = loadAndRunRutModule)]
    pub fn load_and_run_rut_module(&self, source: &str) -> js_sys::Promise {
        let app = self.app.clone();
        let source = source.to_string();
        wasm_bindgen_futures::future_to_promise(async move {
            app.load_and_run_rut_module(&source).await?;
            Ok(JsValue::undefined())
        })
    }

    /// The playground's rut source (the handwritten module + the generated
    /// case registry), concatenated into one loadable module. The site
    /// shell feeds this to `loadAndRunRutModule`.
    #[wasm_bindgen(js_name = playgroundSource)]
    pub fn playground_source(&self) -> String {
        format!("{PLAYGROUND_RUT}\n{CASES_GEN_RUT}")
    }

    /// Return a host-side dev-tool handle. Methods eval the in-engine
    /// `turDevTool` global, returning JSON strings for the host to parse.
    pub fn dev_tool(&self) -> TurDevTool {
        TurDevTool {
            app: self.app.clone(),
        }
    }
}

/// Host-side dev-tool handle, exposed via `TurWebsiteApp.dev_tool()`. Methods
/// return Promises that resolve to JSON strings (the snapshots serialize the
/// engine's Rust state on the worker — see `tur_engine::core::dev`; the
/// underlying RPCs are `async`, so JSON is the simplest transport and the JS
/// host `await`s each call).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct TurDevTool {
    app: tur_wasm::WasmApp,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl TurDevTool {
    /// JSON snapshot of the root node, or `""` if no tree is mounted.
    /// Shape: `{ id, name, label, props, layout:{relative,absolute,width,height,extra?}, queryKey?, children:[{id}, ...] }`.
    #[wasm_bindgen(js_name = elementTree)]
    pub fn element_tree(&self) -> js_sys::Promise {
        self.app.element_tree()
    }

    /// JSON snapshot of a single node by id (full subtree metadata; children
    /// are returned as bare `{id}` handles). Returns `""` if not found.
    #[wasm_bindgen(js_name = getElement)]
    pub fn get_element(&self, id: u32) -> js_sys::Promise {
        self.app.get_element(id)
    }

    /// JSON frame-stats snapshot from the engine realm — the render
    /// performance probe (`turDevTool.frameStats()`):
    /// `{ flushes, paintedFrames, totals, last, lastHost, hostTimingEnabled }`.
    /// `last` is the most recent painted frame's worker-side timing; `lastHost`
    /// the most recent host render-commit timing (`applyUs`/`presentUs`) —
    /// populated only while frame timing is enabled.
    #[wasm_bindgen(js_name = frameStats)]
    pub fn frame_stats(&self) -> js_sys::Promise {
        self.app.frame_stats()
    }

    /// Toggle host-side render-commit timing collection. While on, every
    /// applied frame's `applyUs`/`presentUs` timings are measured and land in
    /// `frameStats().lastHost`. Off by default (zero per-frame overhead).
    #[wasm_bindgen(js_name = setHostFrameTiming)]
    pub fn set_host_frame_timing(&self, enabled: bool) -> js_sys::Promise {
        self.app.set_host_frame_timing(enabled)
    }
}

// (the playground compile service lives in the `tur-playground` crate)
