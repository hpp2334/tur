//! `TurAnimationPlugin` — registers the animation subsystem and the `tur_host`
//! pkg's animation rows.

use std::cell::RefCell;
use std::rc::Rc;

use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::error::TurError;

use crate::flush_hook::AnimationSubsystem;
use crate::manager::AnimationManager;

/// The animation plugin. Registers:
///
///   - the [`AnimationSubsystem`] flush participant (ticks active
///     controllers once per frame),
///   - the `tur_host` pkg's animation rows (via the pkg-extension seam —
///     `el_opacity` / `el_transform` effects, the controller opaque,
///     the tween + curve helpers).
///
/// `TurAnimationPlugin` carries no per-instance state; the animation manager
/// lives inside the registered [`AnimationSubsystem`] for the app's lifetime.
///
/// ## Ordering
///
/// `TurAnimationPlugin` should be registered **immediately after
/// `TurStdPlugin`** in the engine builder call site. The
/// [`Subsystem::flush_pre_layout`](tur_engine::core::subsystem::Subsystem::flush_pre_layout)
/// runs in plugin registration order; the animation
/// subsystem must tick before `flush_reactive` (which it does naturally as
/// the first registered subsystem) so its enqueued `onTick` mutations land
/// in the mutation queue before the next fixed-point iteration drains them.
pub struct TurAnimationPlugin;

impl Default for TurAnimationPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurAnimationPlugin {
    fn register(&self, ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
        // Build the shared animation manager. The manager is shared between
        // the AnimationSubsystem (ticks it once per frame) and the `tur_host` pkg
        // rows (every minted rut controller registers into it).
        let manager: Rc<RefCell<AnimationManager>> = Rc::new(RefCell::new(AnimationManager::new()));
        let clock = ctx.clock();

        // Register the AnimationSubsystem. It runs in registration order
        // relative to other subsystems; this plugin should be added
        // immediately after TurStdPlugin so animation ticks first.
        ctx.register_subsystem(Box::new(AnimationSubsystem::new(manager.clone(), clock)));

        // The `tur_host` pkg rows: pushed as a pkg extension so
        // `RutRuntime::boot` installs them into the pkg (decl + bodies) on
        // both the compile and boot passes. The closure captures the shared
        // manager — a rut controller registers into the SAME registry the
        // subsystem ticks.
        ctx.push_rut_ext(Rc::new(move |cx| {
            crate::rut_rows::install(cx, manager.clone());
            // The animation kit prelude — the authored Opacity / Transform
            // wrappers over the rows above (same ownership law).
            cx.preludes.push(crate::kit::tur_anim_kit_pkg());
        }));

        Ok(())
    }
}
