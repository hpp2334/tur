use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::job::{GenericJob, Job, NativeJob, TimeoutJob};
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::Context;
use boa_engine::JsValue;

#[derive(Default)]
pub struct TimerState {
    next_id: u32,
    cancelled: HashMap<u32, Rc<Cell<bool>>>,
}

impl TimerState {
    pub fn new() -> Self {
        Self::default()
    }

    fn alloc_id(&mut self, cancelled: Rc<Cell<bool>>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.cancelled.insert(id, cancelled);
        id
    }

    pub fn cancel(&mut self, id: u32) {
        if let Some(c) = self.cancelled.get(&id) {
            c.set(true);
        }
    }
}

pub fn register_timer_globals(
    context: &mut Context,
    timer_state: Rc<RefCell<TimerState>>,
    schedule_flush: Rc<Cell<bool>>,
) {
    let state_for_timeout = timer_state.clone();
    let flush_for_timeout = schedule_flush.clone();
    let set_timeout = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let callback = match args.first().and_then(|v| v.as_function()) {
                Some(f) => f,
                None => return Ok(JsValue::undefined()),
            };
            let delay_ms = args
                .get(1)
                .and_then(|v| v.as_number())
                .unwrap_or(0.0)
                .max(0.0) as u64;
            let extra_args: Vec<JsValue> = args.get(2..).map(|s| s.to_vec()).unwrap_or_default();

            let cancelled = Rc::new(Cell::new(false));
            let state_clone = state_for_timeout.clone();
            let id = state_for_timeout.borrow_mut().alloc_id(cancelled.clone());
            let flush_flag = flush_for_timeout.clone();

            let job = NativeJob::new(move |ctx| {
                if cancelled.get() {
                    return Ok(JsValue::undefined());
                }
                state_clone.borrow_mut().cancelled.remove(&id);
                let result = callback.call(&JsValue::undefined(), &extra_args, ctx);
                flush_flag.set(true);
                result
            });

            ctx.enqueue_job(Job::TimeoutJob(TimeoutJob::new(job, delay_ms)));
            Ok(JsValue::from(id))
        })
    };

    let state_for_interval = timer_state.clone();
    let flush_for_interval = schedule_flush.clone();
    let set_interval = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let callback = match args.first().and_then(|v| v.as_function()) {
                Some(f) => f,
                None => return Ok(JsValue::undefined()),
            };
            let delay_ms = args
                .get(1)
                .and_then(|v| v.as_number())
                .unwrap_or(0.0)
                .max(0.0) as u64;
            let extra_args: Vec<JsValue> = args.get(2..).map(|s| s.to_vec()).unwrap_or_default();

            let cancelled = Rc::new(Cell::new(false));
            let id = state_for_interval
                .borrow_mut()
                .alloc_id(cancelled.clone());

            enqueue_interval_tick(
                id,
                callback,
                delay_ms,
                extra_args,
                cancelled,
                flush_for_interval.clone(),
                ctx,
            );

            Ok(JsValue::from(id))
        })
    };

    let state_for_clear = timer_state.clone();
    let clear_timeout = unsafe {
        NativeFunction::from_closure(move |_this, args, _ctx| {
            if let Some(id) = args.first().and_then(|v| v.as_number()) {
                state_for_clear.borrow_mut().cancel(id as u32);
            }
            Ok(JsValue::undefined())
        })
    };

    let state_for_clear_iv = timer_state.clone();
    let clear_interval = unsafe {
        NativeFunction::from_closure(move |_this, args, _ctx| {
            if let Some(id) = args.first().and_then(|v| v.as_number()) {
                state_for_clear_iv.borrow_mut().cancel(id as u32);
            }
            Ok(JsValue::undefined())
        })
    };

    let queue_microtask = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let callback = match args.first().and_then(|v| v.as_function()) {
                Some(f) => f,
                None => return Ok(JsValue::undefined()),
            };
            let realm = ctx.realm().clone();
            let job = GenericJob::new(
                move |ctx| {
                    callback.call(&JsValue::undefined(), &[], ctx)
                },
                realm,
            );
            ctx.enqueue_job(Job::GenericJob(job));
            Ok(JsValue::undefined())
        })
    };

    context
        .register_global_builtin_callable(js_string!("setTimeout"), 2, set_timeout)
        .expect("failed to register setTimeout");
    context
        .register_global_builtin_callable(js_string!("setInterval"), 2, set_interval)
        .expect("failed to register setInterval");
    context
        .register_global_builtin_callable(js_string!("clearTimeout"), 1, clear_timeout)
        .expect("failed to register clearTimeout");
    context
        .register_global_builtin_callable(js_string!("clearInterval"), 1, clear_interval)
        .expect("failed to register clearInterval");
    context
        .register_global_builtin_callable(
            js_string!("queueMicrotask"),
            1,
            queue_microtask,
        )
        .expect("failed to register queueMicrotask");
}

fn enqueue_interval_tick(
    _id: u32,
    callback: boa_engine::object::builtins::JsFunction,
    delay_ms: u64,
    args: Vec<JsValue>,
    cancelled: Rc<Cell<bool>>,
    flush_flag: Rc<Cell<bool>>,
    ctx: &mut Context,
) {
    let job = NativeJob::new(move |ctx| {
        if cancelled.get() {
            return Ok(JsValue::undefined());
        }
        let result = callback.call(&JsValue::undefined(), &args, ctx);
        flush_flag.set(true);

        if !cancelled.get() {
            enqueue_interval_tick(_id, callback, delay_ms, args, cancelled, flush_flag, ctx);
        }
        result
    });

    ctx.enqueue_job(Job::TimeoutJob(TimeoutJob::new(job, delay_ms)));
}
