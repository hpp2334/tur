//! `TurStdPlugin` — the standard widget library plugin.
//!
//! A thin orchestrator: mints the `viewportSize$` engine-environment atom
//! (+ the resize subsystem that publishes it), then calls every domain
//! plugin's surviving `install_xxx`. Elements materialize pure-Rust views
//! (authored through the rut rows in `core::rut_runtime`); there is no JS
//! bridge rail.
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
use crate::core::edgy::reactive::{Readable, Source};
use crate::core::plugin::{Plugin, PluginRegisterContext};
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
        // `viewportSize$` — the canonical engine-environment atom, minted
        // here (plugin-facing recipe): a backing source whose single value
        // home is the INSTANCE store (the seed carries the true initial
        // size), exposed through a derive whose closure reads the backing
        // via a captured instance-store read face — so every read path
        // resolves the same live value, and cache coherence rides the
        // generation rail like any derive. The publisher —
        // `core::screen::ResizeSubsystem`, engine infra wired here — owns
        // the backing + the write rail from here on and publishes on shell
        // `Resize` events. Registered FIRST, so the atom exists before
        // anything can read it and subsystem dispatch order stays:
        // resize → gesture → keyboard → ime → pointer_region.
        //
        // The atom is realm-free end to end (a declaration + a Rust derive
        // closure + a native `{width, height}` map seed — the native-KV
        // substrate holds the value without a realm).
        let bridge = ctx.reactive();
        let initial = ctx.viewport();
        let backing: Source<crate::core::edgy::Value> = bridge.decl_source(
            crate::core::edgy::Value::map([
                ("width", crate::core::edgy::Value::Num(initial.0)),
                ("height", crate::core::edgy::Value::Num(initial.1)),
            ]),
        );
        let read_face = bridge.read_only();
        let _viewport_size_handle =
            bridge.build_derive(move |_read| Ok(read_face.read(Readable::from(backing))));
        ctx.register_subsystem(Box::new(ResizeSubsystem::new(backing, bridge, initial)));
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
        install_text(ctx)?;
        install_scroll(ctx)?;
        install_gesture(ctx)?;
        install_input(ctx)?;
        // CompositedTransformTarget/Follower + createLayerLink + tracking
        // subsystem (the link registry rides the plugin-state channel).
        install_composited_transform(ctx)?;
        // Virtual apps — the `VirtualAppSubsystem` (status/frame
        // consumption, layout-driven resize, input forwarding) + the shared
        // per-instance `VirtualState` on the plugin-state channel.
        virtual_app::install_virtual_app(ctx)?;

        Ok(())
    }
}
