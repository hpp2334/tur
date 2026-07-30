//! `TurStdPlugin` — the standard widget library plugin.
//!
//! Registers the `tur:std` JS module (widget factories, controllers,
//! color bridge) plus the engine's input-event subsystems (resize, gesture,
//! keyboard, ime, pointer region — the latter four registered inside their
//! respective `install_xxx` calls).
//!
//! `TurStdPlugin` carries no per-instance state. Backend injection
//! (clipboard, http, cursor) happens via `TurEngineBuilder::capability(...)`
//! and dedicated plugins (`TurClipboardPlugin`, `TurNetPlugin`). Animation
//! (`createAnimationController`, `AnimatedContainer`/`AnimatedOpacity`/
//! `AnimatedPositioned`, `Tween`, `ColorTween`) is provided by the separate
//! `tur-animation` crate via `tur_animation::TurAnimationPlugin`. The visual
//! effects `Opacity` / `Transform` ship as part of `tur:std`.
//!
//! ## Architecture
//!
//! - Internal plugins (`control_flow`, `console`, `focus`, `gesture`,
//!   `input`, `layout`, `lifecycle`, `text`, `scroll`, `lazy_container`)
//!   live in sibling modules under `builtin_plugins/` and expose one
//!   `install_xxx(ctx)` each. This file is the orchestrator that calls
//!   them and merges their `FnEntry`s into the single `tur:std`
//!   module.
//! - Inlined feature bundles (`image`, `scroll`, `lazy_container`, `text`)
//!   follow the same `install_xxx` pattern and are also merged into
//!   `tur:std`.
//! - Engine-owned reactive-edge bridge (`source`/`derive`/`mutate`/`get`/
//!   `set`/`view`) + render mount + async task primitives stay in `core::*`
//!   (renderer/async/edgy infra, not plugin affinity).

use crate::builtin_plugins::{
    console::install_console, control_flow::install_control_flow, effects::install_effects,
    focus::install_focus, gesture::install_gesture, image::install_image, input::install_input,
    layout::composited_transform::install_composited_transform, layout::enums,
    layout::install_layout, lazy_container::install_lazy_container, lifecycle::install_lifecycle,
    scroll::install_scroll, text::install_text,
};
use crate::core::app::render;
use crate::core::async_::task;
use crate::core::js_runtime::helpers::{ConstEntry, FnEntry};
use crate::core::plugin::{Plugin, PluginContext};
use crate::core::screen::ResizeSubsystem;
use crate::error::TurError;
use boa_engine::native_function::NativeFunction;

/// The standard widget library plugin. Registers the `tur:std`
/// module (widget factories, controllers, color bridge), plus the
/// input-event subsystems (gesture, keyboard, ime, resize, pointer region).
///
/// `TurStdPlugin` carries no per-instance state. Backend injection
/// (clipboard, http, cursor) happens via `TurEngineBuilder::capability(...)`
/// and dedicated plugins (`TurClipboardPlugin`, `TurNetPlugin`). Animation
/// (`createAnimationController`, `AnimatedContainer`/`AnimatedOpacity`/
/// `AnimatedPositioned`, `Tween`, `ColorTween`) is provided by the separate
/// `tur-animation` crate via `tur_animation::TurAnimationPlugin`. The visual
/// effects `Opacity` / `Transform` ship as part of `tur:std`.
pub struct TurStdPlugin;

impl Default for TurStdPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurStdPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        // Subsystem registration order matters: gesture → keyboard → ime →
        // resize → pointer_region. The resize handler is renderer infra
        // (`core::screen::ResizeSubsystem`); the gesture / pointer-region /
        // keyboard / ime subsystems are registered by their respective
        // `install_xxx` calls below.
        ctx.register_subsystem(Box::new(ResizeSubsystem));
        // Note: ClipboardPlatformSubsystem (embedder paste → engine-internal
        // paste forwarding) and ClipboardWriteSubsystem (Cmd+C/X → backend)
        // both live in `builtin_plugins::clipboard` (TurClipboardPlugin) —
        // registered there along with the JS bridge so the embedder wires
        // the clipboard backend through a single `.capability(...)` call.

        let mut std_fns: Vec<FnEntry> = Vec::new();
        let mut std_closures: Vec<(&str, usize, NativeFunction)> = Vec::new();
        // Inlined feature plugins (text, image, scroll, lazy_container used
        // to be external `tur-*` crates; they now live inside
        // `builtin_plugins/` and follow the same `install_xxx` pattern as
        // the original domain plugins).
        std_fns.extend(install_text(ctx)?);
        std_fns.extend(install_image(ctx)?);
        std_fns.extend(install_scroll(ctx)?);
        std_fns.extend(install_lazy_container(ctx)?);
        // Global `console.log` / `.warn` / `.error` / `.info` / `.debug`.
        std_fns.extend(install_console(ctx)?);
        // Engine-owned builtin plugins. Each plugin's `install_xxx` returns
        // the bridge entries for every element + JS-facing primitive in that
        // plugin (e.g. `install_layout` returns Column/Row/Expanded/Stack/
        // Positioned/Container/SizedBox). Subsystem-bearing plugins
        // (gesture, input) also register their subsystems inside their
        // `install_xxx`.
        std_fns.extend(install_control_flow(ctx)?);
        std_fns.extend(crate::core::edgy::fns());
        std_fns.extend(install_focus(ctx)?);
        std_fns.extend(install_gesture(ctx)?);
        std_fns.extend(install_input(ctx)?);
        std_fns.extend(install_effects(ctx)?);
        std_fns.extend(install_layout(ctx)?);
        // CompositedTransformTarget/Follower + createLayerLink + tracking
        // subsystem. Returns element FnEntries + a state-capturing closure
        // (createLayerLink), both merged into `tur:std`.
        let (ct_fns, ct_closures) = install_composited_transform(ctx)?;
        std_fns.extend(ct_fns);
        std_closures.extend(ct_closures);
        std_fns.extend(install_lifecycle(ctx)?);
        // Render mount + async task primitives stay in `core::app::render`
        // and `core::async_::task` (renderer/async infra, not element-plugin
        // affinity).
        std_fns.extend(render::fns());
        std_fns.extend(task::fns());
        std_fns.extend(crate::core::render::brush::bridge::fns());

        let mut std_consts: Vec<ConstEntry> = Vec::new();
        let js_ctx_value = ctx.js_ctx_value.clone();
        std_consts.extend(crate::core::render::brush::bridge::consts(ctx.boa_mut(), js_ctx_value));
        std_consts.extend(enums::consts(ctx.boa_mut()));
        // Engine-owned reactive source exposing the live canvas size as
        // `{width, height}` (CSS pixels). The engine syncs it each frame in
        // `TurAppInternal::flush`; JS reads it via `get(viewportSize$).width`.
        std_consts.push(("viewportSize$", ctx.viewport_size.clone()));

        ctx.register_module("tur:std", std_fns, std_closures, std_consts);

        Ok(())
    }
}
