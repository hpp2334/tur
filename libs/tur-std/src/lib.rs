//! Standard widget library plugin for tur.
//!
//! Provides [`TurStdPlugin`], which registers the `tur:std` JS module
//! (widget factories, controllers, color bridge) plus the engine's
//! input-event handlers (gesture, keyboard, ime, paste, resize, wheel, scroll
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
//! - Most elements, controllers, handlers, and bridge fns live in `tur-engine`
//!   (they're the engine's building blocks). This crate is a thin registration
//!   layer that wires them into the `tur:std` module.
//! - Text feature (Text/Input elements, `TextEditingController`,
//!   `UndoController`, `EnsureCaretVisibleHandler`, `extract_layout_data`) is
//!   pulled in from the standalone `tur-text` crate via
//!   [`tur_text::install_text_feature`] and merged into `std_fns` here — so
//!   from JS's perspective Text/Input ship as part of `tur:std`. The
//!   engine retains only the paint/layout contract types (`TextLayoutData`,
//!   `FontManager`).
//! - Scroll feature (`ScrollView`, `Scrollbar`, `ScrollController`,
//!   `ScrollSubsystem`) is pulled in from the standalone `tur-scroll` crate
//!   via [`tur_scroll::install_scroll_feature`]. Lazy-list feature
//!   (`LazyList`, `LazyListController`) is pulled in from `tur-lazy-container`
//!   via [`tur_lazy_container::install_lazy_container_feature`]. Both are
//!   merged into `std_fns` here. The engine retains only the event protocol
//!   (`AppEvent::Scroll` / `ScrollTo` / `ScrollOverscroll`) and `WheelEvent`
//!   primitive.
//! - Cursor-backend capability types (`CursorBackend`, `CursorCap`,
//!   `NoopCursor`) live in `tur_engine::core::platform` and are re-exported at
//!   the `tur_engine::` crate root — import them from there, not from here.

use tur_engine::core::bridge::helpers::ConstEntry;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::core::bridge::{reactive, render};
use tur_engine::error::TurError;

/// The standard widget library plugin. Registers the `tur:std`
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
        use tur_engine::core::bridge::{color_fns, enums};
        use tur_engine::core::bridge::helpers::FnEntry;
        use tur_engine::core::handlers;

        ctx.register_subsystem(Box::new(handlers::gesture::GestureSubsystem::new()));
        ctx.register_subsystem(Box::new(handlers::keyboard::KeyboardSubsystem));
        ctx.register_subsystem(Box::new(handlers::ime::ImeSubsystem));
        ctx.register_subsystem(Box::new(handlers::resize::ResizeSubsystem));
        ctx.register_subsystem(Box::new(handlers::pointer_region::PointerSubsystem::new()));
        // Note: ClipboardPlatformSubsystem (embedder paste → engine-internal
        // paste forwarding) and ClipboardWriteSubsystem (Cmd+C/X → backend)
        // both live in `tur-clipboard-capability` (TurClipboardPlugin) —
        // registered there along with the JS bridge so the embedder wires
        // the clipboard backend through a single `.capability(...)` call.

        let mut std_fns: Vec<FnEntry> = Vec::new();
        // Text feature (Text/Input elements, TextEditingController /
        // UndoController classes, ensure-caret-visible post-handler) is
        // installed into `tur:std` rather than as a separate plugin.
        // tur-text owns all text logic; the engine keeps only the
        // paint/layout contract types (`TextLayoutData`, `FontManager`).
        std_fns.extend(tur_text::install_text_feature(ctx)?);
        // Image feature (Image element, createImageResource /
        // createSvgResource, PNG/JPEG/SVG decode) lives in the standalone
        // `tur-image` crate; installed into `tur:std` here. The
        // engine retains only the paint/layout contract types
        // (`ImageResourceId`, `ImageResourceMap`, `ImageResource`).
        std_fns.extend(tur_image::install_image_feature(ctx)?);
        // Scroll feature (ScrollView, Scrollbar, ScrollController,
        // ScrollSubsystem) and lazy-list feature (LazyList,
        // LazyListController) live in the standalone `tur-scroll` and
        // `tur-lazy-container` crates; installed into `tur:std`
        // here. The engine retains only the `AppEvent::Scroll*` protocol and
        // `WheelEvent` primitive.
        std_fns.extend(tur_scroll::install_scroll_feature(ctx)?);
        std_fns.extend(tur_lazy_container::install_lazy_container_feature(ctx)?);
        std_fns.extend(reactive::fns());
        std_fns.extend(render::fns());
        std_fns.extend(tur_engine::core::bridge::task::fns());
        std_fns.extend(color_fns::fns());
        std_fns.extend(tur_engine::elements::container::bridge::fns());
        std_fns.extend(tur_engine::elements::flex::bridge::fns());
        std_fns.extend(tur_engine::elements::flex_item::bridge::fns());
        std_fns.extend(tur_engine::elements::stack::bridge::fns());
        std_fns.extend(tur_engine::elements::positioned::bridge::fns());
        std_fns.extend(tur_engine::elements::pointer_interact::bridge::fns());
        std_fns.extend(tur_engine::elements::mouse_region::bridge::fns());
        std_fns.extend(tur_engine::elements::condition::bridge::fns());
        std_fns.extend(tur_engine::elements::switch::bridge::fns());
        std_fns.extend(tur_engine::elements::each::bridge::fns());
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

        ctx.register_module("tur:std", std_fns, vec![], std_consts);

        Ok(())
    }
}
