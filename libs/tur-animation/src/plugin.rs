//! `TurAnimationPlugin` — registers the animation subsystem, the
//! `AnimationController` boa class, and the `tur:animation` JS module.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use boa_engine::class::Class;
use boa_engine::native_function::NativeFunction;
use boa_engine::JsValue;
use boa_gc::{Finalize, Trace};
use tur_engine::core::js_runtime::helpers::{ConstEntry, FnEntry};
use tur_engine::core::edgy::mutation::PendingMutationInvocationQueue;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;

use crate::controller::AnimationController;
use crate::effects;
use crate::flush_hook::AnimationSubsystem;
use crate::manager::AnimationManager;

/// The animation plugin. Registers:
///
///   - the `AnimationController` boa class (constructed via
///     `createAnimationController` — direct `new AnimationController(...)`
///     would skip manager + mutation-queue injection),
///   - the [`AnimationSubsystem`] flush participant (ticks active
///     controllers once per frame),
///   - the `Opacity` and `Transform` visual-effect elements (bridge fns),
///   - the `tur:animation` JS module combining the native bridge
///     fns with the JS-defined implicit-animation widgets
///     (`AnimatedContainer`, `AnimatedOpacity`, `AnimatedPositioned`,
///     `Tween`, `ColorTween`).
///
/// `TurAnimationPlugin` carries no per-instance state; the animation manager
/// lives inside the registered [`AnimationSubsystem`] for the app's lifetime.
///
/// ## Ordering
///
/// `TurAnimationPlugin` should be registered **immediately after
/// `TurStdPlugin`** in the engine builder call site. The
/// [`Subsystem::flush`] runs in plugin registration order; the animation
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
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        // 1. Register the AnimationController boa class. JS constructs via
        //    `createAnimationController` (the closure registered below), which
        //    injects the manager + mutation_queue handles; a direct
        //    `new AnimationController(...)` would silently drop callbacks.
        ctx.register_class::<AnimationController>()
            .map_err(|e| TurError::Other(format!("failed to register AnimationController: {e}")))?;

        // 2. Build the shared animation manager + capture engine-owned handles.
        //    The manager is shared between:
        //    - the AnimationSubsystem (ticks it once per frame),
        //    - the createAnimationController closure (registers new controllers
        //      into it on `forward()` / `reverse()`).
        let manager: Rc<RefCell<AnimationManager>> = Rc::new(RefCell::new(AnimationManager::new()));
        let mutation_queue = ctx.mutation_queue();
        let clock = ctx.clock();

        // 3. Register the AnimationSubsystem. It runs in registration order
        //    relative to other subsystems; this plugin should be added
        //    immediately after TurStdPlugin so animation ticks first.
        ctx.register_subsystem(Box::new(AnimationSubsystem::new(
            manager.clone(),
            clock,
        )));

        // 4. Register the hidden internal native module `tur:animation/native`.
        //    The consumer-facing `tur:animation` JS source imports
        //    from here to access the native bridge fns.
        let mut native_fns: Vec<FnEntry> = Vec::new();
        native_fns.extend(effects::bridge::fns()); // Opacity, Transform (ctx-bound)

        let create_animation_controller = build_create_animation_controller(
            manager.clone(),
            mutation_queue.clone(),
        );

        ctx.register_module(
            "tur:animation/native",
            native_fns,
            vec![("createAnimationController", 1, create_animation_controller)],
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

/// Captures stashed inside the `createAnimationController` JS closure. Held
/// inside boa's GC heap (the closure is wrapped in a `Gc`), so the type
/// must implement `Trace` — but it owns only pure-Rust state (no `Gc` /
/// `GcRefCell`), so the trace is empty.
#[derive(Clone, Trace, Finalize)]
#[boa_gc(unsafe_empty_trace)]
struct AnimationCtrlCaptures {
    manager: Rc<RefCell<AnimationManager>>,
    mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
}

/// Build the `createAnimationController` native closure. Captures the shared
/// animation manager + the engine-wide mutation queue so each newly-built
/// controller can register itself on `forward()` / `reverse()` and enqueue
/// `onTick` / `onEnd` callbacks for deferred dispatch.
fn build_create_animation_controller(
    manager: Rc<RefCell<AnimationManager>>,
    mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
) -> NativeFunction {
    NativeFunction::from_copy_closure_with_captures(
        move |_this, args, state, context| {
            let mgr = state.manager.clone();
            let mq = state.mutation_queue.clone();
            let data = AnimationController::data_constructor(
                &JsValue::undefined(),
                &args[0..],
                context,
            )?;
            let obj = AnimationController::from_data(data, context)?;
            if let Some(mut ctrl) = obj.downcast_mut::<AnimationController>() {
                ctrl.set_animation_manager(mgr);
                ctrl.set_mutation_queue(mq);
            }
            Ok(obj.upcast().clone().into())
        },
        AnimationCtrlCaptures {
            manager,
            mutation_queue,
        },
    )
}
