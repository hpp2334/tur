//! The lazy-font plugin: `TurFontPlugin` — the capability + rows + kit.
//!
//! The split: the engine's font rail (`core::fonts::FontRail`) owns the
//! declared-face registry and registrations (the shared collection); the
//! [`Fonts`] capability owns "where the bytes come from" (the host's
//! `FontDataProvider` — a CDN catalog on wasm, APK assets on android, stub
//! bytes in tests). The plugin feature-detects the capability at register
//! and skips the rows when absent (the `TurNetPlugin` pattern), so plugin
//! sets without a provider never see (or compile) the font kit.

mod capability;
mod kit;
pub mod rut_rows;

pub use capability::{FontDataProvider, Fonts};

use std::rc::Rc;

use crate::core::plugin::{Plugin, PluginRegisterContext};
use crate::error::TurError;

/// The lazy-font plugin. Register via
/// `TurRuntimeBuilder::plugin(TurFontPlugin)` together with the
/// `Fonts::new(provider)` capability; without the capability the plugin is
/// a no-op (rows + kit skipped).
pub struct TurFontPlugin;

impl Default for TurFontPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurFontPlugin {
    fn register(&self, ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
        // Optional capability: if no font provider is registered, skip
        // pushing the font rows (and the kit).
        if !ctx.capability().contains::<Fonts>() {
            tracing::info!("TurFontPlugin: no Fonts capability registered; skipping the font rows");
            return Ok(());
        }
        ctx.push_rut_ext(Rc::new(|cx: &mut crate::core::rut_runtime::RutPkgCx<'_>| {
            cx.preludes
                .push(crate::builtin_plugins::font::kit::font_kit_pkg());
            rut_rows::install(cx);
        }));
        Ok(())
    }
}
