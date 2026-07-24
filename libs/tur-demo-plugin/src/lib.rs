//! Demo-only plugin for tur.
//!
//! Provides [`TurDemoPlugin`], which registers the `tur-ext/demo-helper` JS
//! module — swc-backed compiler services (`transpileTsx`, `tokenizeTsx`,
//! `generateAst`) and, when the `web-io` feature is enabled, browser file IO
//! (`pickFile`, `saveFile`). These are playground-specific helpers that depend
//! on swc and (for file IO) the browser DOM; they are not part of the core
//! engine surface.
//!
//! The plugin carries no per-instance state. [`resolve_pending_picks`] (only
//! available with `web-io`) is a frame-time hook the wasm embedder calls to
//! drain pending `pickFile` resolutions on its `after_frame` callback (where a
//! `&mut Context` is available).
//!
//! Non-browser embedders (e.g. Android via `tur-android`) build with
//! `default-features = false` to compile the swc compiler fns only — the
//! `pickFile`/`saveFile` exports are then registered as no-op stubs so the
//! same `dist/impl.js` playground bundle resolves on every host.

mod compiler;
mod host_fns;

#[cfg(feature = "web-io")]
mod file_io;

use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;

#[cfg(feature = "web-io")]
pub use file_io::resolve_pending_picks;

// Re-export the swc compiler primitives so downstream embedders can use them
// directly (matches the surface tur-wasm used to expose).
pub use compiler::{
    generate_ast, highlight_tsx, tokenize_tsx, transpile_tsx, AstNode, AstNodeKind,
    ImportSpecifierInfo, TokenSpan,
};

/// The demo-helper plugin. Registers the `tur-ext/demo-helper` module with
/// swc-backed compiler services and (with the `web-io` feature) browser file
/// IO. Playground-only — not part of the core engine API.
pub struct TurDemoPlugin;

impl Default for TurDemoPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurDemoPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let service_fns = host_fns::build_host_service_fns();
        let exports: Vec<(String, boa_engine::NativeFunction, usize)> = service_fns
            .into_iter()
            .map(|(n, f, l)| (n.to_string(), f, l))
            .collect();
        // The browser file-IO fns are only available with `web-io`; on other
        // hosts (Android) register no-op stubs under the same names so the
        // playground's `import * as Host from "tur-ext/demo-helper"` resolves
        // and `Host.pickFile`/`Host.saveFile` are callable (they just do
        // nothing — `saveFile`'s absence is already handled gracefully by
        // `github-viewer`, and `pickFile` has no JS consumer).
        #[cfg(feature = "web-io")]
        let file_fns = file_io::build_file_io_fns();
        #[cfg(not(feature = "web-io"))]
        let file_fns = stub_file_io_fns();
        let exports: Vec<_> = exports
            .into_iter()
            .chain(file_fns.into_iter().map(|(n, f, l)| (n.to_string(), f, l)))
            .collect();
        ctx.register_host_module("tur-ext/demo-helper", exports);
        Ok(())
    }
}

/// No-op `pickFile`/`saveFile` stubs for non-browser hosts. Registered so the
/// playground bundle's `Host` namespace always has both exports, even though
/// they have no native implementation outside the browser. `saveFile`'s
/// absence is already tolerated by `github-viewer`'s guard; `pickFile` has no
/// JS consumer in the playground.
#[cfg(not(feature = "web-io"))]
fn stub_file_io_fns() -> Vec<(&'static str, boa_engine::NativeFunction, usize)> {
    use boa_engine::{JsValue, NativeFunction};

    let pick = NativeFunction::from_copy_closure(|_this, _args, _ctx| {
        // No file picker on this host; nothing to resolve.
        Ok(JsValue::undefined())
    });
    let save = NativeFunction::from_copy_closure(|_this, _args, _ctx| Ok(JsValue::undefined()));
    vec![("pickFile", pick, 1), ("saveFile", save, 2)]
}
