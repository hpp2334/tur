use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::class::{Class, ClassBuilder};
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};
use tur_shared::{AnimationCurve, Tween};

use crate::core::bridge::color::extract_color;
use crate::core::bridge::{BoaOpaque, TurJsContext, TurNodeHandle};
use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationStatus {
    Stopped,
    Forward,
    Reverse,
    Completed,
}

#[derive(Debug, Clone)]
pub struct TweenEntry {
    pub property: String,
    pub tween: Tween,
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
    pub(crate) on_end: Option<JsFunction>,

    tweens: Vec<TweenEntry>,
    element_id: Option<ElementNodeId>,
    start_time_ms: Option<u64>,

    pub(crate) element_tree: Option<Rc<RefCell<ElementTree>>>,
    pub(crate) dirty_flag: Option<Rc<Cell<bool>>>,
    pub(crate) handle: Option<JsObject>,
    pub(crate) animation_manager:
        Option<Rc<RefCell<crate::core::animation::AnimationManager>>>,
}

fn extract_callable(value: &JsValue) -> Option<JsFunction> {
    value.as_object().and_then(JsFunction::from_object)
}

fn extract_callable_from_opts(
    opts: &JsObject,
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
            on_end: None,
            tweens: Vec::new(),
            element_id: None,
            start_time_ms: None,
            element_tree: None,
            dirty_flag: None,
            handle: None,
            animation_manager: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            AnimationStatus::Forward | AnimationStatus::Reverse
        )
    }

    fn node_id(&self) -> Option<ElementNodeId> {
        let handle_obj = self.handle.as_ref()?;
        let handle = BoaOpaque::<TurNodeHandle>::wrap(handle_obj)?;
        Some(handle.id)
    }

    pub fn tick(&mut self, now_ms: u64) -> bool {
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

        let element_id = self.element_id;
        let mut tree = match self.element_tree.as_ref() {
            Some(rc) => rc.borrow_mut(),
            None => return false,
        };

        for entry in &self.tweens {
            let value = entry.tween.lerp(eased_t);
            if let Some(node_id) = element_id {
                if let Some(node) = tree.get_mut(node_id) {
                    if let Some(ref mut element) = node.element {
                        element.apply_animated(&entry.property, value);
                    }
                }
                if crate::core::animation::AnimationManager::property_affects_layout(
                    &entry.property,
                ) {
                    tree.mark_dirty(node_id);
                } else {
                    tree.mark_dirty_paint(node_id);
                }
            }
        }
        drop(tree);

        if completed {
            self.current_iteration = max_iterations;
            self.status = AnimationStatus::Completed;
            self.value = if direction > 0.0 { 1.0 } else { 0.0 };
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
            on_end = extract_callable_from_opts(&opts, "onEnd", ctx);
        }

        let mut ctrl = Self::new(duration_ms, curve);
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

                if let Some(mgr_rc) = &ctrl.animation_manager {
                    mgr_rc.borrow_mut().register_controller(obj.clone());
                }
                if let Some(dirty) = &ctrl.dirty_flag {
                    dirty.set(true);
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

                if let Some(mgr_rc) = &ctrl.animation_manager {
                    mgr_rc.borrow_mut().register_controller(obj.clone());
                }
                if let Some(dirty) = &ctrl.dirty_flag {
                    dirty.set(true);
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

        class.method(
            js_string!("setTweens"),
            1,
            NativeFunction::from_fn_ptr(|this, args, ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<AnimationController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                let tweens_obj = args.get_or_undefined(0).as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("setTweens expects an object")
                })?;

                let keys = tweens_obj.own_property_keys(ctx).map_err(|e| {
                    JsNativeError::typ().with_message(format!("{e}"))
                })?;

                let mut entries = Vec::new();
                for prop_key in keys {
                    let prop_key_str = match &prop_key {
                        boa_engine::property::PropertyKey::String(s) => {
                            s.to_std_string_escaped()
                        }
                        _ => continue,
                    };
                    let prop_val = tweens_obj.get(prop_key, ctx).map_err(|e| {
                        JsNativeError::typ().with_message(format!("{e}"))
                    })?;
                    let Some(prop_obj) = prop_val.as_object() else {
                        continue;
                    };

                    let tween = parse_tween_obj(&prop_obj, ctx)?;
                    entries.push(TweenEntry {
                        property: prop_key_str,
                        tween,
                    });
                }

                ctrl.tweens = entries;
                ctrl.element_id = ctrl.node_id();

                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("_attach"),
            2,
            NativeFunction::from_fn_ptr(|this, args, _| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<AnimationController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                if let Some(handle_obj) = args.get_or_undefined(0).as_object() {
                    if BoaOpaque::<TurNodeHandle>::wrap(&handle_obj).is_some() {
                        ctrl.handle = Some(handle_obj.clone());
                        ctrl.element_id = ctrl.node_id();
                    }
                }

                if let Some(ctx_obj) = args.get_or_undefined(1).as_object() {
                    if let Some(js_ctx) = BoaOpaque::<TurJsContext>::wrap(&ctx_obj) {
                        ctrl.element_tree = Some(js_ctx.element_tree.clone());
                        ctrl.dirty_flag = Some(js_ctx.dirty.clone());
                        ctrl.animation_manager =
                            Some(js_ctx.animation_manager.clone());
                    }
                }

                Ok(JsValue::undefined())
            }),
        );

        Ok(())
    }
}

fn parse_tween_obj(obj: &JsObject, ctx: &mut Context) -> JsResult<Tween> {
    let begin_val = obj.get(js_string!("begin"), ctx).map_err(|e| {
        JsNativeError::typ().with_message(format!("tween missing 'begin': {e}"))
    })?;
    let end_val = obj.get(js_string!("end"), ctx).map_err(|e| {
        JsNativeError::typ().with_message(format!("tween missing 'end': {e}"))
    })?;
    let type_hint = obj
        .get(js_string!("type"), ctx)
        .ok()
        .and_then(|v| v.as_string().map(|s| s.to_std_string_escaped()));

    match type_hint.as_deref() {
        Some("color") => {
            let b = extract_color(&begin_val, ctx).ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("tween type 'color' but begin is not a color")
            })?;
            let e = extract_color(&end_val, ctx).ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("tween type 'color' but end is not a color")
            })?;
            Ok(Tween::Color { begin: b, end: e })
        }
        Some("float") | None => {
            match (begin_val.as_number(), end_val.as_number()) {
                (Some(b), Some(e)) => Ok(Tween::Float { begin: b, end: e }),
                _ => {
                    let b = extract_color(&begin_val, ctx).ok_or_else(|| {
                        JsNativeError::typ().with_message(
                            "cannot determine tween type from begin/end values",
                        )
                    })?;
                    let e = extract_color(&end_val, ctx).ok_or_else(|| {
                        JsNativeError::typ().with_message(
                            "cannot determine tween type from begin/end values",
                        )
                    })?;
                    Ok(Tween::Color { begin: b, end: e })
                }
            }
        }
        Some(other) => Err(JsNativeError::typ()
            .with_message(format!("unknown tween type: {other}"))
            .into()),
    }
}
