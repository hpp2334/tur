use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::class::{Class, ClassBuilder};
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};
use tur_shared::Curve;

use crate::core::animation::event::{AnimationEndEvent, AnimationTickEvent};
use crate::core::animation::AnimationManager;
use crate::core::edgy_event::{EdgyMutation, PendingMutationInvocationQueue, extract_mutation_from_opts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationStatus {
    Stopped,
    Forward,
    Reverse,
    Completed,
    Paused,
}

/// Repeat policy for an [`AnimationController`]. Mirrors the user-facing
/// JS API: `repeat(count)` accepts a positive integer or the string
/// `"infinite"`. Internally we model both as a single enum so the tick
/// math has a single point that gates completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    /// Play exactly `n` iterations, then transition to `Completed`.
    Finite(u64),
    /// Loop forever. `tick_compute` never sets status to `Completed`;
    /// `onEnd` never fires. The value cycles through `[0, 1]` via the
    /// existing `rem_euclid(1.0)` wrap math.
    Infinite,
}

impl Default for RepeatMode {
    fn default() -> Self {
        RepeatMode::Finite(1)
    }
}

#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct AnimationController {
    duration_ms: u64,
    curve: Curve,
    value: f64,
    status: AnimationStatus,
    repeat_mode: RepeatMode,
    current_iteration: u64,
    /// The `value` captured when this animation segment started (set by
    /// `forward`/`reverse`/`resume`/`seek`). Used by `tick` to compute the
    /// current value relative to the start, which makes `speed`, `seek`,
    /// and `pause`/`resume` work correctly.
    value_at_start: f64,
    /// Time multiplier (default 1.0). Mutated by `set_speed`. Higher values
    /// play faster; 0.5 = half speed, 2.0 = double speed.
    speed: f64,
    /// The direction the controller was traveling before `pause()` — used
    /// by `resume()` to pick up where it left off.
    paused_direction: Option<AnimationStatus>,
    /// Atom-backed callback handle for `onTick`. Resolved via the reactive
    /// store at flush time (just like every other event handler), so the
    /// callback runs after all `RefMut` borrows are released.
    on_tick: Option<EdgyMutation<AnimationTickEvent>>,
    /// Atom-backed callback handle for `onEnd`. Same dispatch path as
    /// `on_tick`.
    on_end: Option<EdgyMutation<AnimationEndEvent>>,
    start_time_ms: Option<u64>,
    animation_manager: Option<Rc<RefCell<AnimationManager>>>,
    /// The engine-wide mutation queue. Set by `tur_create_animation_controller`
    /// at construction time. Used to defer `onTick` / `onEnd` invocations to
    /// the next flush — never invoke these callbacks synchronously while
    /// holding a `RefMut` on the controller.
    mutation_queue: Option<Rc<RefCell<PendingMutationInvocationQueue>>>,
}

impl AnimationController {
    pub fn new(duration_ms: u64, curve: Curve) -> Self {
        Self {
            duration_ms,
            curve,
            value: 0.0,
            status: AnimationStatus::Stopped,
            repeat_mode: RepeatMode::default(),
            current_iteration: 0,
            value_at_start: 0.0,
            speed: 1.0,
            paused_direction: None,
            on_tick: None,
            on_end: None,
            start_time_ms: None,
            animation_manager: None,
            mutation_queue: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            AnimationStatus::Forward | AnimationStatus::Reverse
        )
    }

    pub fn set_animation_manager(&mut self, mgr: Rc<RefCell<AnimationManager>>) {
        self.animation_manager = Some(mgr);
    }

    /// Set the engine-wide mutation queue. Called once at construction by
    /// `tur_create_animation_controller`. Required for `onTick` / `onEnd`
    /// dispatch — if `None`, callbacks are silently dropped.
    pub fn set_mutation_queue(&mut self, queue: Rc<RefCell<PendingMutationInvocationQueue>>) {
        self.mutation_queue = Some(queue);
    }

    /// Enqueue an `onTick(eased_t)` invocation on the mutation queue. The
    /// callback fires during the next `flush_pending_mutations` pass, after
    /// any active `RefMut` borrow on this controller (or any other) is
    /// released. Safe to call while holding a `RefMut` — this only clones
    /// an `AtomId` and pushes a `Box<dyn EventArg>` onto a separate `RefCell`.
    fn enqueue_tick(&self, eased_t: f64) {
        if let (Some(queue), Some(m)) = (&self.mutation_queue, self.on_tick) {
            queue.borrow_mut().push(m, AnimationTickEvent(eased_t));
        }
    }

    /// Enqueue an `onEnd()` invocation. See `enqueue_tick` for the dispatch
    /// rationale.
    fn enqueue_end(&self) {
        if let (Some(queue), Some(m)) = (&self.mutation_queue, self.on_end) {
            queue.borrow_mut().push(m, AnimationEndEvent);
        }
    }

    /// Recompute `start_time_ms` so the next `tick` continues from the
    /// current `value`. Used after `seek` and `set_speed` to keep the math
    /// consistent.
    fn rebase_start_to_current_value(&mut self, now_ms: u64) {
        if self.duration_ms == 0 {
            self.start_time_ms = Some(now_ms);
            return;
        }
        // We have value = value_at_start + direction * (elapsed * speed / duration).
        // Solve for elapsed: elapsed = (value - value_at_start) * duration / (direction * speed).
        let direction: f64 = if self.status == AnimationStatus::Forward {
            1.0
        } else {
            -1.0
        };
        let delta = self.value - self.value_at_start;
        let elapsed_ms = if (direction * self.speed).abs() < 1e-9 {
            0.0
        } else {
            (delta * self.duration_ms as f64) / (direction * self.speed)
        };
        // elapsed_ms may be negative if value moved backwards — clamp to >=0
        // to keep start_time in the past.
        let elapsed_ms = elapsed_ms.max(0.0);
        self.start_time_ms = Some(now_ms.saturating_sub(elapsed_ms as u64));
    }

    /// Compute one tick of the animation: update `value` and `status` based
    /// on the elapsed time, and **enqueue** (not fire) the `onTick` / `onEnd`
    /// callbacks on the mutation queue. The callbacks fire during the next
    /// `flush_pending_mutations` pass, after all `RefMut` borrows are
    /// released — this avoids the boa `BorrowError` that would otherwise
    /// occur if a JS callback accessed the controller (e.g. `ctrl.status`).
    ///
    /// Returns `true` if the controller ticked (active and had a start time),
    /// `false` if it was idle.
    pub fn tick_compute(&mut self, now_ms: u64) -> bool {
        if !self.is_active() {
            return false;
        }
        let Some(start) = self.start_time_ms else {
            return false;
        };

        let direction: f64 = if self.status == AnimationStatus::Forward {
            1.0
        } else {
            -1.0
        };

        let elapsed_ms = now_ms.saturating_sub(start) as f64;
        let scaled_elapsed_ms = elapsed_ms * self.speed;
        let progress_delta = scaled_elapsed_ms / self.duration_ms.max(1) as f64;

        let new_value = self.value_at_start + direction * progress_delta;

        let (max_iterations, infinite) = match self.repeat_mode {
            RepeatMode::Finite(n) => (n, false),
            RepeatMode::Infinite => (u64::MAX, true),
        };
        let completed = !infinite
            && if direction > 0.0 {
                new_value >= max_iterations as f64
            } else {
                // Reverse starts at value_at_start (typically 1.0) and decreases.
                new_value <= (1.0 - max_iterations as f64)
            };

        let t = if completed {
            if direction > 0.0 { 1.0 } else { 0.0 }
        } else if infinite || max_iterations > 1 {
            let frac = new_value.rem_euclid(1.0);
            if direction > 0.0 { frac } else { 1.0 - frac }
        } else {
            new_value.clamp(0.0, 1.0)
        };

        self.value = t;
        self.current_iteration = if completed {
            max_iterations
        } else if infinite {
            // For infinite mode, current_iteration grows unboundedly; cap
            // at u64::MAX to avoid overflow. Useful only for diagnostics —
            // the user reads `value`, not `current_iteration`.
            (new_value.max(0.0).floor() as u64).saturating_add(0)
        } else if max_iterations > 1 {
            (new_value.max(0.0).floor() as u64).min(max_iterations)
        } else {
            0
        };

        let eased_t = self.curve.transform(t);

        if completed {
            self.status = AnimationStatus::Completed;
            self.value = if direction > 0.0 { 1.0 } else { 0.0 };
            self.paused_direction = None;
        }

        // Enqueue callbacks — they fire later, outside the RefMut borrow.
        self.enqueue_tick(eased_t);
        if completed {
            self.enqueue_end();
        }
        true
    }
}

impl Class for AnimationController {
    const NAME: &'static str = "AnimationController";
    const LENGTH: usize = 1;

    fn data_constructor(
        _new_target: &JsValue,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<Self> {
        let mut duration_ms = 300u64;
        let mut curve = Curve::Linear;
        let mut on_tick: Option<EdgyMutation<AnimationTickEvent>> = None;
        let mut on_end: Option<EdgyMutation<AnimationEndEvent>> = None;
        let mut repeat_mode = RepeatMode::default();

        if let Some(opts) = args.get_or_undefined(0).as_object() {
            if let Ok(val) = opts.get(js_string!("duration"), ctx) {
                if let Some(n) = val.as_number() {
                    duration_ms = n as u64;
                }
            }
            if let Ok(val) = opts.get(js_string!("curve"), ctx) {
                if let Some(s) = val.as_string() {
                    curve = s
                        .to_std_string_escaped()
                        .parse()
                        .unwrap_or(Curve::Linear);
                }
            }
            if let Ok(val) = opts.get(js_string!("repeat"), ctx) {
                repeat_mode = parse_repeat_value(&val);
            }
            on_tick = extract_mutation_from_opts(&opts, "onTick", ctx);
            on_end = extract_mutation_from_opts(&opts, "onEnd", ctx);
        }

        let mut ctrl = Self::new(duration_ms, curve);
        ctrl.on_tick = on_tick;
        ctrl.on_end = on_end;
        ctrl.repeat_mode = repeat_mode;
        Ok(ctrl)
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        macro_rules! controller_getter {
            ($name:expr, $body:expr) => {
                let getter = NativeFunction::from_fn_ptr($body)
                    .to_js_function(class.context().realm());
                class.accessor(
                    js_string!($name),
                    Some(getter),
                    None,
                    Attribute::default(),
                );
            };
        }

        controller_getter!("value", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj
                .downcast_ref::<AnimationController>()
                .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;
            Ok(JsValue::from(ctrl.value))
        });

        controller_getter!("status", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj
                .downcast_ref::<AnimationController>()
                .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;
            let s = match ctrl.status {
                AnimationStatus::Stopped => "stopped",
                AnimationStatus::Forward => "forward",
                AnimationStatus::Reverse => "reverse",
                AnimationStatus::Completed => "completed",
                AnimationStatus::Paused => "paused",
            };
            Ok(JsValue::from(js_string!(s)))
        });

        controller_getter!("duration", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj
                .downcast_ref::<AnimationController>()
                .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;
            Ok(JsValue::from(ctrl.duration_ms as f64))
        });

        controller_getter!("speed", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj
                .downcast_ref::<AnimationController>()
                .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;
            Ok(JsValue::from(ctrl.speed))
        });

        class.method(
            js_string!("forward"),
            0,
            NativeFunction::from_fn_ptr(|this, _args, ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<AnimationController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                ctrl.status = AnimationStatus::Forward;
                ctrl.start_time_ms = Some(ctx.clock().now().millis_since_epoch());
                ctrl.current_iteration = 0;
                ctrl.value = 0.0;
                ctrl.value_at_start = 0.0;
                ctrl.paused_direction = None;

                // Enqueue (not fire) so the callback runs in the next flush,
                // outside the active `RefMut` borrow on `ctrl`.
                ctrl.enqueue_tick(0.0);

                if let Some(mgr_rc) = &ctrl.animation_manager {
                    mgr_rc.borrow_mut().register_controller(obj.clone());
                }

                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("reverse"),
            0,
            NativeFunction::from_fn_ptr(|this, _args, ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<AnimationController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                ctrl.status = AnimationStatus::Reverse;
                ctrl.start_time_ms = Some(ctx.clock().now().millis_since_epoch());
                ctrl.current_iteration = 0;
                ctrl.value = 1.0;
                ctrl.value_at_start = 1.0;
                ctrl.paused_direction = None;

                ctrl.enqueue_tick(1.0);

                if let Some(mgr_rc) = &ctrl.animation_manager {
                    mgr_rc.borrow_mut().register_controller(obj.clone());
                }

                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("stop"),
            0,
            NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<AnimationController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                ctrl.status = AnimationStatus::Stopped;
                ctrl.start_time_ms = None;
                ctrl.paused_direction = None;

                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("pause"),
            0,
            NativeFunction::from_fn_ptr(|this, _args, ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<AnimationController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                // Only pause if currently running. Capture the direction so
                // `resume()` knows which way to continue.
                if ctrl.is_active() {
                    // Tick once with the current time so `value` reflects the
                    // moment of pause before we freeze the start time. The
                    // tick enqueues any callbacks on the mutation queue
                    // (fired later, outside this borrow).
                    let now = ctx.clock().now().millis_since_epoch();
                    let _ = ctrl.tick_compute(now);
                    ctrl.paused_direction = Some(ctrl.status);
                    ctrl.status = AnimationStatus::Paused;
                    ctrl.start_time_ms = None;
                }

                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("resume"),
            0,
            NativeFunction::from_fn_ptr(|this, _args, ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<AnimationController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                if ctrl.status != AnimationStatus::Paused {
                    return Ok(JsValue::undefined());
                }
                let direction = ctrl
                    .paused_direction
                    .unwrap_or(AnimationStatus::Forward);
                ctrl.status = direction;
                ctrl.value_at_start = ctrl.value;
                ctrl.start_time_ms = Some(ctx.clock().now().millis_since_epoch());
                ctrl.paused_direction = None;

                if let Some(mgr_rc) = &ctrl.animation_manager {
                    mgr_rc.borrow_mut().register_controller(obj.clone());
                }

                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("seek"),
            1,
            NativeFunction::from_fn_ptr(|this, args, ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<AnimationController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                let t = args.get_or_undefined(0).as_number().unwrap_or(0.0).clamp(0.0, 1.0);
                ctrl.value = t;
                ctrl.value_at_start = t;

                if ctrl.is_active() {
                    // Re-base start_time so the next tick continues from t.
                    let now = ctx.clock().now().millis_since_epoch();
                    ctrl.rebase_start_to_current_value(now);
                }

                ctrl.enqueue_tick(t);

                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("setSpeed"),
            1,
            NativeFunction::from_fn_ptr(|this, args, ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<AnimationController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                let s = args.get_or_undefined(0).as_number().unwrap_or(1.0);
                if s <= 0.0 {
                    return Err(boa_engine::JsError::from(JsNativeError::range()
                        .with_message("setSpeed: speed must be positive")));
                }

                if ctrl.is_active() {
                    // Tick once at the old speed to align `value` (enqueues
                    // any pending callbacks), then apply the new speed and
                    // re-base the start time.
                    let now = ctx.clock().now().millis_since_epoch();
                    let _ = ctrl.tick_compute(now);
                    ctrl.speed = s;
                    ctrl.value_at_start = ctrl.value;
                    ctrl.start_time_ms = Some(now);
                } else {
                    ctrl.speed = s;
                }

                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("repeat"),
            0,
            NativeFunction::from_fn_ptr(|this, args, _ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<AnimationController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                ctrl.repeat_mode = parse_repeat_value(args.get_or_undefined(0));
                ctrl.current_iteration = 0;

                Ok(JsValue::undefined())
            }),
        );

        Ok(())
    }
}

/// Parse a JS value into a `RepeatMode`. Accepts:
///   - A positive number → `Finite(n)` (0 / negative clamped to 1)
///   - The string `"infinite"` → `Infinite`
///   - `undefined` / `null` / unrecognized → `Finite(1)` (default)
fn parse_repeat_value(val: &JsValue) -> RepeatMode {
    if let Some(s) = val.as_string() {
        if s.to_std_string_escaped() == "infinite" {
            return RepeatMode::Infinite;
        }
        return RepeatMode::default();
    }
    if let Some(n) = val.as_number() {
        if n <= 0.0 || !n.is_finite() {
            return RepeatMode::Finite(1);
        }
        return RepeatMode::Finite(n as u64);
    }
    RepeatMode::default()
}
