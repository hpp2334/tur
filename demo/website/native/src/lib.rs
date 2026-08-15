//! WebAssembly entry for the tur website.
//!
//! `tur-wasm` is a reusable embedder lib (it owns all the DOM wiring + engine
//! glue but exports no `#[wasm_bindgen]` surface and pulls in no playground
//! code). This crate is the website's *own* `.so`: it wraps `tur-wasm`'s
//! [`tur_wasm::WasmRuntime`] + [`tur_wasm::WasmApp`] builders and adds the
//! playground-only [`tur_demo_plugin::TurDemoPlugin`] (swc TS compiler). JS
//! imports `TurWebsiteApp` from the generated `tur_website.js`.
//!
//! Mirrors the Android split: `tur-android` (pure rlib) vs `demo/compose/native`
//! (the app's own cdylib that adds the demo plugin set).

// Everything is wasm32-only — on a host `cargo check --workspace` this is an
// empty (but compiling) cdylib.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// One-time wasm init (panic hook + tracing). Called automatically on module
/// instantiation via the `#[wasm_bindgen(start)]` attribute.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_entry() {
    tur_wasm::init();
}

/// A running tur website app. Construct via [`TurWebsiteApp::create`] (full
/// viewport) or [`TurWebsiteApp::create_in`] (embedded in a container element).
/// Load a view bundle (e.g. the playground-view `impl.js`) via
/// `loadAndRunModule`.
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
        Self::create_internal(None)
    }

    /// Embed the canvas inside the element with the given id.
    pub fn create_in(container_id: String) -> js_sys::Promise {
        Self::create_internal(Some(container_id))
    }

    fn create_internal(container_id: Option<String>) -> js_sys::Promise {
        wasm_bindgen_futures::future_to_promise(async move {
            // Build the shared runtime once with the demo plugin.
            let runtime = tur_wasm::WasmRuntime::create(tur_wasm::WasmRuntimeConfig {
                configure: Box::new(|b| b.plugin(tur_demo_plugin::TurDemoPlugin)),
                pools: Vec::new(),
            })?;
            // Spawn an isolated DOM-wired instance from it.
            let app = tur_wasm::WasmApp::create(
                &runtime,
                tur_wasm::WasmAppConfig {
                    container_id,
                    after_frame: None,
                    pool: None,
                },
            )
            .await?;
            Ok(JsValue::from(TurWebsiteApp {
                _runtime: runtime,
                app,
            }))
        })
    }

    /// Evaluate `js_source` as an ES module (supports real
    /// `import { ... } from "tur:..."`, resolved by the engine's module
    /// loader), then render. Used to load the playground-view bundle.
    ///
    /// Async: returns a Promise that resolves once the module finishes
    /// loading + evaluating.
    #[wasm_bindgen(js_name = loadAndRunModule)]
    pub fn load_and_run_module(&self, js_source: &str) -> js_sys::Promise {
        let app = self.app.clone();
        let js_source = js_source.to_string();
        wasm_bindgen_futures::future_to_promise(async move {
            app.load_and_run_module(&js_source).await?;
            Ok(JsValue::undefined())
        })
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
/// return Promises that resolve to JSON strings (the data originates inside
/// the boa engine — a separate JS realm — and the underlying RPCs are now
/// `async`, so JSON is the simplest cross-realm transport and the JS host
/// `await`s each call).
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
}
