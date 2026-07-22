//! Demo-only plugin for tur.
//!
//! Provides [`TurDemoPlugin`], which registers the `tur-ext/demo-helper` JS
//! module — swc-backed compiler services (`transpileTsx`, `tokenizeTsx`,
//! `generateAst`) and browser file IO (`pickFile`, `saveFile`). These are
//! playground-specific helpers that depend on swc and the browser DOM; they
//! are not part of the core engine surface.
//!
//! The plugin carries no per-instance state. [`resolve_pending_picks`] is a
//! frame-time hook the wasm embedder calls to drain pending `pickFile`
//! resolutions on its `after_frame` callback (where a `&mut Context` is
//! available).

mod compiler;
mod file_io;
mod host_fns;

use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;

pub use file_io::resolve_pending_picks;

// Re-export the swc compiler primitives so downstream embedders can use them
// directly (matches the surface tur-wasm used to expose).
pub use compiler::{
    generate_ast, highlight_tsx, tokenize_tsx, transpile_tsx, AstNode, AstNodeKind,
    ImportSpecifierInfo, TokenSpan,
};

/// The demo-helper plugin. Registers the `tur-ext/demo-helper` module with
/// swc-backed compiler services and browser file IO. Playground-only — not
/// part of the core engine API.
pub struct TurDemoPlugin;

impl Default for TurDemoPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurDemoPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let service_fns = host_fns::build_host_service_fns();
        let file_fns = file_io::build_file_io_fns();
        let exports: Vec<(String, boa_engine::NativeFunction, usize)> = service_fns
            .into_iter()
            .chain(file_fns)
            .map(|(n, f, l)| (n.to_string(), f, l))
            .collect();
        ctx.register_host_module("tur-ext/demo-helper", exports);
        Ok(())
    }
}
