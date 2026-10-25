//! `TurStdPlugin` — the standard widget library plugin.
//!
//! A thin orchestrator: mints the `viewport_width` / `viewport_height`
//! engine-environment atoms (+ the resize subsystem that publishes them),
//! then calls every domain plugin's surviving `install_xxx`. Elements
//! materialize pure-Rust views (authored through the rut rows in
//! `core::rut_runtime`); there is no JS bridge rail.
//!
//! `TurStdPlugin` carries no per-instance state. Backend injection
//! (clipboard, http, cursor) happens via `TurRuntimeBuilder::capability(...)`
//! and dedicated plugins (`TurClipboardPlugin`, `TurNetPlugin`). Animation
//! (`createAnimationController`, `AnimatedContainer`/`AnimatedOpacity`/
//! `AnimatedPositioned`, `Tween`, `ColorTween`) is provided by the separate
//! `tur-animation` crate via `tur_animation::TurAnimationPlugin`.

use crate::builtin_plugins::{
    gesture::install_gesture,
    input::install_input,
    layout::composited_transform::install_composited_transform,
    scroll::install_scroll,
    text::install_text,
    virtual_app,
};
use crate::core::edgy::reactive::Source;
use crate::core::edgy::value::Value;
use crate::core::plugin::{Plugin, PluginRegisterContext};
use crate::core::rut_runtime::ViewportEnv;
use crate::core::screen::ResizeSubsystem;
use crate::error::TurError;

/// The standard widget library plugin. Mints the engine-environment atoms
/// and registers the input-event + feature subsystems through the domain
/// plugins' `install_xxx` fns.
///
/// `TurStdPlugin` carries no per-instance state. Backend injection
/// (clipboard, http, cursor) happens via `TurRuntimeBuilder::capability(...)`
/// and dedicated plugins (`TurClipboardPlugin`, `TurNetPlugin`).
pub struct TurStdPlugin;

impl Default for TurStdPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurStdPlugin {
    fn register(&self, ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
        // The viewport env atoms — the canonical engine-environment recipe,
        // minted here (plugin-facing): two f64 backing sources
        // (`viewport_width` / `viewport_height`, CSS-pixel logical units)
        // whose single value home is the INSTANCE store (the seeds carry
        // the true initial size). Their ids slot as plugin state
        // (`ViewportEnv`), where the `env` rows resolve them per instance —
        // every read path (rut's `Readable` handles) addresses the same
        // live atoms. The publisher — `core::screen::ResizeSubsystem`,
        // engine infra wired here — owns the backings + the write rail
        // from here on and publishes on shell `Resize` events. Registered
        // FIRST, so the atoms exist before anything can read them and
        // subsystem dispatch order stays:
        // resize → gesture → keyboard → ime → pointer_region.
        //
        // The atoms are realm-free end to end (declarations + a native f64
        // seed each — the native-KV substrate holds the values without a
        // realm).
        let bridge = ctx.reactive();
        let initial = ctx.viewport();
        let backing_w: Source<f64> = bridge.decl_source(Value::Num(initial.0));
        let backing_h: Source<f64> = bridge.decl_source(Value::Num(initial.1));
        ctx.define_plugin_state::<ViewportEnv>(std::rc::Rc::new(ViewportEnv {
            w: backing_w.id().0 as u64,
            h: backing_h.id().0 as u64,
        }));
        ctx.register_subsystem(Box::new(ResizeSubsystem::new(
            backing_w,
            backing_h,
            bridge,
            initial,
        )));
        // Note: ClipboardPlatformSubsystem (embedder paste → engine-internal
        // paste forwarding) and ClipboardWriteSubsystem (Cmd+C/X → backend)
        // both live in `builtin_plugins::clipboard` (TurClipboardPlugin) —
        // registered there so the embedder wires the clipboard backend
        // through a single `.capability(...)` call.

        // Feature plugins — each `install_xxx` registers its subsystems /
        // plugin state / rut-ext rows (in contract order). Inlined feature
        // bundles (text, scroll) register their subsystems here too;
        // subsystem-bearing plugins (gesture, input) register theirs inside
        // their `install_xxx`.
        crate::builtin_plugins::layout::install_layout(ctx)?;
        crate::builtin_plugins::control_flow::install_control_flow(ctx)?;
        crate::builtin_plugins::lazy_container::install_lazy_container(ctx)?;
        crate::builtin_plugins::image::install_image(ctx)?;
        crate::builtin_plugins::lifecycle::install_lifecycle(ctx)?;
        install_text(ctx)?;
        install_scroll(ctx)?;
        install_gesture(ctx)?;
        install_input(ctx)?;
        // CompositedTransformTarget/Follower + createLayerLink + tracking
        // subsystem (the link registry rides the plugin-state channel) +
        // its families' rows.
        install_composited_transform(ctx)?;
        // Virtual apps — the `VirtualAppSubsystem` (status/frame
        // consumption, layout-driven resize, input forwarding) + the shared
        // per-instance `VirtualState` on the plugin-state channel + its
        // family's rows.
        virtual_app::install_virtual_app(ctx)?;

        // The kit — the authored builder surface (a rut source module).
        // Lives OUTSIDE core/ (the layering law): core never knows it; the
        // standard bundle assembly registers it as a compile-session
        // prelude so `use tur_kit::{…}` resolves in every module the
        // standard plugin set compiles.
        ctx.push_rut_ext(std::rc::Rc::new(|cx: &mut crate::core::rut_runtime::RutPkgCx<'_>| {
            cx.preludes.push(crate::kit::tur_kit_pkg());
            // The std json offering (rut's `json` + its dep closure) —
            // same registration shape, the offered-set lane: modules
            // spell `use json::{ decodeJson };` (builtin_plugins/std_json
            // owns the vendoring; rut-std ships the *_host bodies).
            crate::builtin_plugins::std_json::push_offering(cx);
        }));

        Ok(())
    }
}
