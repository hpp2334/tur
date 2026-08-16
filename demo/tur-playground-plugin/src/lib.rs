//! Playground-only plugin for tur.
//!
//! Provides [`TurPlaygroundPlugin`], which registers the `tur-ext/demo-helper`
//! JS module — swc-backed compiler services (`transpileTsx`, `tokenizeTsx`,
//! `generateAst`). These are playground-specific helpers that depend on swc;
//! they are not part of the core engine surface.
//!
//! File IO (`pick` / `saveFile`) used to live here as a browser-only hack; it
//! has moved to the `tur:filepicker` capability (`tur-filepicker-capability` +
//! the `tur-filepicker-wasm` / `tur-filepicker-native` backends), which is
//! Promise-based, swappable, and available on every host.
//!
//! The plugin carries no per-instance state.

mod compiler;
mod host_fns;

use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;

// Re-export the swc compiler primitives so downstream embedders can use them
// directly (matches the surface tur-wasm used to expose).
pub use compiler::{
    AstNode, AstNodeKind, ImportSpecifierInfo, TokenSpan, generate_ast, highlight_tsx,
    tokenize_tsx, transpile_tsx,
};

/// The playground plugin. Registers the `tur-ext/demo-helper` module with
/// swc-backed compiler services. Playground-only — not part of the core
/// engine API.
pub struct TurPlaygroundPlugin;

impl Default for TurPlaygroundPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurPlaygroundPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let service_fns = host_fns::build_host_service_fns();
        let exports: Vec<(String, boa_engine::NativeFunction, usize)> = service_fns
            .into_iter()
            .map(|(n, f, l)| (n.to_string(), f, l))
            .collect();
        ctx.register_native_module("tur-ext/demo-helper", exports);
        Ok(())
    }
}
