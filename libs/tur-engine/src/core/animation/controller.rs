use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::class::{Class, ClassBuilder};
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsFunction;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};
use tur_shared::AnimationCurve;

use crate::core::animation::AnimationManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationStatus {
    Stopped,
    Forward,
    Reverse,
    Completed,
}

#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct AnimationController {
    pub(crate) duration_ms: u64,
    pub(crate) curve: AnimationCurve,
    pub(crate) value: f64,
    pub(crate) status: AnimationStatus,
    pub(crate) repeat_count: Option<u64>,
    pub(crate) current_iteration: u64,
    on_tick: Option<JsFunction>,
    on_end: Option<JsFunction>,
    start_time_ms: Option<u64>,
    animation_manager: Option<Rc<RefCell<AnimationManager>>>,
}

fn extract_callable(value: &JsValue) -> Option<JsFunction> {
    value.as_object().and_then(JsFunction::from_object)
}

fn extract_callable_from_opts(
    opts: &boa_engine::object::JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<JsFunction> {
    let val = opts.get(js_string!(key), ctx).ok()?;
    extract_callable(&val)
}

impl AnimationController {
    pub fn new(duration_ms: u64, curve: AnimationCurve) -> Self {
        Self {
            duration_ms,
            curve,
            value: 0.0,
            status: AnimationStatus::Stopped,
            repeat_count: None,
            current_iteration: 0,
            on_tick: None,
            on_end: None,
            start_time_ms: None,
            animation_manager: None,
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

    pub fn tick(&mut self, now_ms: u64, ctx: &mut Context) -> bool {
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

        let elapsed = now_ms.saturating_sub(start);
        let raw_progress = elapsed as f64 / self.duration_ms as f64;

        let max_iterations = self.repeat_count.unwrap_or(1);
        let (iteration, frac) = if raw_progress >= max_iterations as f64 {
            (max_iterations, 1.0)
        } else {
            let iter = (raw_progress as u64).min(max_iterations - 1);
            let frac = raw_progress - iter as f64;
            (iter + 1, frac)
        };

        let completed = frac >= 1.0 && iteration >= max_iterations;
        let t = if completed {
            1.0f64
        } else {
            frac.min(1.0)
        };

        let t = if direction > 0.0 { t } else { 1.0 - t };
        self.value = t.clamp(0.0, 1.0);
        let eased_t = self.curve.apply(self.value);

        if let Some(ref callback) = self.on_tick {
            let _ = callback.call(&JsValue::undefined(), &[JsValue::from(eased_t)], ctx);
        }

        if completed {
            self.current_iteration = max_iterations;
            self.status = AnimationStatus::Completed;
            self.value = if direction > 0.0 { 1.0 } else { 0.0 };
            if let Some(ref callback) = self.on_end {
                let _ = callback.call(&JsValue::undefined(), &[], ctx);
            }
        } else {
            self.current_iteration = iteration;
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
        let mut curve = AnimationCurve::Linear;
        let mut on_tick = None;
        let mut on_end = None;

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
                        .unwrap_or(AnimationCurve::Linear);
                }
            }
            on_tick = extract_callable_from_opts(&opts, "onTick", ctx);
            on_end = extract_callable_from_opts(&opts, "onEnd", ctx);
        }

        let mut ctrl = Self::new(duration_ms, curve);
        ctrl.on_tick = on_tick;
        ctrl.on_end = on_end;
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

                let eased_0 = ctrl.curve.apply(0.0);
                if let Some(ref callback) = ctrl.on_tick {
                    let _ = callback.call(&JsValue::undefined(), &[JsValue::from(eased_0)], ctx);
                }

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

                let eased_1 = ctrl.curve.apply(1.0);
                if let Some(ref callback) = ctrl.on_tick {
                    let _ = callback.call(&JsValue::undefined(), &[JsValue::from(eased_1)], ctx);
                }

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

                let count = args.get_or_undefined(0).as_number().map(|n| n as u64);
                ctrl.repeat_count = count;
                ctrl.current_iteration = 0;

                Ok(JsValue::undefined())
            }),
        );

        Ok(())
    }
}
