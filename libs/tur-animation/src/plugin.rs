//! `TurAnimationPlugin` — registers the animation subsystem, the
//! `AnimationController` boa class, and the `tur:animation` JS module.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use boa_engine::JsValue;
use boa_engine::class::Class;
use tur_engine::core::js_runtime::helpers::{ConstEntry, Ptr};
use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::error::TurError;

use crate::controller::AnimationController;
use crate::flush_hook::AnimationSubsystem;
use crate::manager::AnimationManager;

/// The animation plugin. Registers:
///
///   - the `AnimationController` boa class (constructed via
///     `createAnimationController` — direct `new AnimationController(...)`
///     would skip manager + mutation-queue injection),
///   - the [`AnimationSubsystem`] flush participant (ticks active
///     controllers once per frame),
///   - the `tur:animation` JS module combining the native `createAnimationController`
///     bridge fn with the JS-defined implicit-animation widgets
///     (`AnimatedContainer`, `AnimatedOpacity`, `AnimatedPositioned`,
///     `Tween`, `ColorTween`). The visual effects `Opacity` / `Transform`
///     used by those widgets are imported from `tur:std`.
///
/// `TurAnimationPlugin` carries no per-instance state; the animation manager
/// lives inside the registered [`AnimationSubsystem`] for the app's lifetime.
///
/// ## Ordering
///
/// `TurAnimationPlugin` should be registered **immediately after
/// `TurStdPlugin`** in the engine builder call site. The
/// [`Subsystem::flush_pre_layout`] runs in plugin registration order; the animation
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
    fn register(&self, ctx: &mut PluginRegisterContext<'_>) -> Result<(), TurError> {
        // 1. Register the AnimationController boa class. JS constructs via
        //    `createAnimationController` (the closure registered below), which
        //    injects the manager + mutation_queue handles; a direct
        //    `new AnimationController(...)` would silently drop callbacks.
        ctx.register_class::<AnimationController>()
            .map_err(|e| TurError::Other(format!("failed to register AnimationController: {e}")))?;

        // 2. Build the shared animation manager. The manager is shared
        //    between:
        //    - the AnimationSubsystem (ticks it once per frame),
        //    - the createAnimationController bridge fn (registers new
        //      controllers into it on `forward()` / `reverse()`) — reached
        //      through the register-phase plugin-state channel, so the fn is
        //      a plain ctx-bound pointer (no closures).
        let manager: Rc<RefCell<AnimationManager>> = Rc::new(RefCell::new(AnimationManager::new()));
        let clock = ctx.clock();

        // 3. Register the AnimationSubsystem. It runs in registration order
        //    relative to other subsystems; this plugin should be added
        //    immediately after TurStdPlugin so animation ticks first.
        ctx.register_subsystem(Box::new(AnimationSubsystem::new(manager.clone(), clock)));

        // 4. Register the hidden internal native module `tur:animation/native`.
        //    The consumer-facing `tur:animation` JS source imports
        //    from here to access the native bridge fns. All exports are
        //    ctx-bound `FnEntry`s (the AGENTS rule: plugins provide fns).
        ctx.define_plugin_state(Rc::new(AnimationHostState {
            manager: manager.clone(),
        }));

        ctx.register_module(
            "tur:animation/native",
            vec![(
                "createAnimationController",
                2,
                tur_create_animation_controller as Ptr,
            )],
            Vec::<ConstEntry>::new(),
        );

        // 5. Register the consumer-facing `tur:animation` JS module.
        //    It imports the native fns from `tur:animation/native`, imports
        //    the reactive + layout primitives from `tur:std`, and
        //    defines + exports the implicit-animation widgets.
        ctx.register_js_module(
            "tur:animation",
            include_str!("../js/index.js"),
            Path::new("libs/tur-animation/js/index.js"),
        )?;

        Ok(())
    }
}

/// Per-instance plugin state: the shared animation manager, held by the
/// subsystem and readable by the `createAnimationController` bridge fn
/// through the instance ctx (the register-phase plugin-state channel).
pub(crate) struct AnimationHostState {
    manager: Rc<RefCell<AnimationManager>>,
}

/// `createAnimationController(opts)` — a plain ctx-bound fn pointer (ctx at
/// `args[0]`, user opts at `args[1]`). Builds the controller and injects the
/// shared manager (plugin state) + the engine-wide mutation queue (off the
/// instance ctx), so each controller can register itself on `forward()` /
/// `reverse()` and enqueue `onTick` / `onEnd` callbacks for deferred
/// dispatch.
fn tur_create_animation_controller(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut boa_engine::Context,
) -> boa_engine::JsResult<JsValue> {
    let js_ctx = tur_engine::core::js_runtime::helpers::extract_js_ctx(args)?;
    let manager = js_ctx
        .plugin_state::<AnimationHostState>()
        .ok_or_else(|| {
            boa_engine::JsNativeError::typ()
                .with_message("animation plugin not registered on this instance")
        })?
        .manager
        .clone();
    let mutation_queue = js_ctx.mutation_queue.clone();
    let data = AnimationController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    let obj = AnimationController::from_data(data, context)?;
    if let Some(mut ctrl) = obj.downcast_mut::<AnimationController>() {
        ctrl.set_animation_manager(manager);
        ctrl.set_mutation_queue(mutation_queue);
    }
    Ok(obj.upcast().clone().into())
}
