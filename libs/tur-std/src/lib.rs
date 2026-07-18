//! Standard widget library plugin for tur.
//!
//! Provides [`TurStdPlugin`], which registers the `builtin:tur/std` JS module
//! (widget factories, controllers, color bridge) plus the engine's
//! input-event handlers (gesture, keyboard, ime, resize, wheel, scroll
//! chaining, pointer region, scroll-to).
//!
//! `TurStdPlugin` carries no per-instance state. Backend injection
//! (clipboard, http, cursor) happens via
//! [`tur_engine::TurEngineBuilder::capability`] and dedicated plugins
//! (`TurClipboardPlugin`, `TurNetPlugin`). Animation (`AnimationController`,
//! `Opacity`, `Transform`, `AnimatedContainer`/`AnimatedOpacity`/
//! `AnimatedPositioned`) is provided by the separate `tur-animation` crate
//! via [`tur_animation::TurAnimationPlugin`].
//!
//! ## Architecture
//!
//! - All elements, controllers, handlers, and bridge fns live in `tur-engine`
//!   (they're the engine's building blocks). This crate is a thin registration
//!   layer that wires them into the `builtin:tur/std` module.
//! - Cursor-backend capability types (`CursorBackend`, `CursorCap`,
//!   `NoopCursor`) live in `tur_engine::core::platform` and are re-exported at
//!   the `tur_engine::` crate root — import them from there, not from here.

use tur_engine::core::bridge::helpers::{ConstEntry, FnEntry};
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::core::bridge::{reactive, render};
use tur_engine::error::TurError;

/// The standard widget library plugin. Registers the `builtin:tur/std`
/// module (widget factories, controllers, color bridge), plus the
/// input-event handlers (gesture, keyboard, ime, resize, wheel, scroll
/// chaining, pointer region, paste).
///
/// `TurStdPlugin` carries no per-instance state. Backend injection
/// (clipboard, http, cursor) happens via `TurEngineBuilder::capability(...)`
/// and dedicated plugins (`TurClipboardPlugin`, `TurNetPlugin`). Animation
/// (`Opacity`, `Transform`, `createAnimationController`, `AnimatedContainer`
/// /`AnimatedOpacity`/`AnimatedPositioned`) is provided by the separate
/// `tur-animation` crate via `tur_animation::TurAnimationPlugin`.
pub struct TurStdPlugin;

impl Default for TurStdPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurStdPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        use tur_engine::core::scroll::ScrollController;
        use tur_engine::core::text::controller::{TextEditingController, UndoController};
        use tur_engine::core::bridge::{color_fns, enums};
        use tur_engine::core::handlers;
        use tur_engine::elements::lazy_list::LazyListController;

        ctx.register_class::<TextEditingController>()
            .expect("failed to register TextEditingController");
        ctx.register_class::<UndoController>()
            .expect("failed to register UndoController");
        ctx.register_class::<ScrollController>()
            .expect("failed to register ScrollController");
        ctx.register_class::<LazyListController>()
            .expect("failed to register LazyListController");

        ctx.register_handler(Box::new(handlers::gesture::GestureAppHandler::new()));
        ctx.register_handler(Box::new(handlers::keyboard::KeyboardAppHandler));
        ctx.register_handler(Box::new(handlers::ime::ImeAppHandler));
        ctx.register_handler(Box::new(handlers::resize::ResizeHandler));
        ctx.register_handler(Box::new(handlers::pointer_region::PointerRegionAppHandler::new()));
        ctx.register_handler(Box::new(handlers::wheel::WheelAppHandler));
        ctx.register_handler(Box::new(handlers::scroll_chaining::ScrollChainingHandler));
        ctx.register_handler(Box::new(handlers::scroll_to::ScrollToHandler));
        // Note: ClipboardPasteHandler and ClipboardWriteHandler have moved
        // to `tur-clipboard-capability` (TurClipboardPlugin) — they're
        // registered there along with the JS bridge so the embedder wires
        // the clipboard backend through a single `.capability(...)` call.

        let mut std_fns: Vec<FnEntry> = Vec::new();
        std_fns.extend(reactive::fns());
        std_fns.extend(render::fns());
        std_fns.extend(tur_engine::core::bridge::task::fns());
        std_fns.extend(color_fns::fns());
        std_fns.extend(tur_engine::elements::container::bridge::fns());
        std_fns.extend(tur_engine::elements::flex::bridge::fns());
        std_fns.extend(tur_engine::elements::flex_item::bridge::fns());
        std_fns.extend(tur_engine::elements::stack::bridge::fns());
        std_fns.extend(tur_engine::elements::positioned::bridge::fns());
        std_fns.extend(tur_engine::elements::paragraph::bridge::fns());
        std_fns.extend(tur_engine::elements::editable_text::bridge::fns());
        std_fns.extend(tur_engine::elements::image::bridge::fns());
        std_fns.extend(tur_engine::elements::pointer_interact::bridge::fns());
        std_fns.extend(tur_engine::elements::mouse_region::bridge::fns());
        std_fns.extend(tur_engine::elements::condition::bridge::fns());
        std_fns.extend(tur_engine::elements::switch::bridge::fns());
        std_fns.extend(tur_engine::elements::each::bridge::fns());
        std_fns.extend(tur_engine::elements::lazy_list::bridge::fns());
        std_fns.extend(tur_engine::elements::scroll_view::bridge::fns());
        std_fns.extend(tur_engine::elements::scrollbar::bridge::fns());
        std_fns.extend(tur_engine::elements::fragment::bridge::fns());
        std_fns.extend(tur_engine::elements::focusable::bridge::fns());
        std_fns.extend(tur_engine::elements::lifecycle::bridge::fns());
        std_fns.extend(tur_engine::elements::readable_subscribe::bridge::fns());

        let mut std_consts: Vec<ConstEntry> = Vec::new();
        let js_ctx_value = ctx.js_ctx_value.clone();
        std_consts.extend(color_fns::consts(ctx.boa_mut(), js_ctx_value));
        std_consts.extend(enums::consts(ctx.boa_mut()));
        // Engine-owned reactive source exposing the live canvas size as
        // `{width, height}` (CSS pixels). The engine syncs it each frame in
        // `TurAppInternal::flush`; JS reads it via `get(viewportSize$).width`.
        std_consts.push(("viewportSize$", ctx.viewport_size.clone()));

        ctx.register_module("builtin:tur/std", std_fns, vec![], std_consts);

        Ok(())
    }
}
