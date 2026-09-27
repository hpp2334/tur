//! Dev-tool bridge: `__tur._dev_tool_element_tree` /
//! `__tur._dev_tool_get_element`, plus the public `turDevTool` global that
//! wraps them.
//!
//! The public JS API is `turDevTool.elementTree()` /
//! `turDevTool.getElement(id)`. Those are thin wrappers (registered as a
//! small JS snippet at init) that forward to the underscore-prefixed
//! natives on `__tur`, auto-passing `__tur.__ctx` — matching the layering
//! every other `__tur.*` API uses.

use boa_engine::object::JsObject;
use boa_engine::object::builtins::JsArray;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, js_string};

use crate::core::app::frame_stats::{FrameTiming, HostFrameTiming};
use crate::core::elements::{DevNodeData, TraceValue};
use crate::core::js_runtime::helpers::extract_js_ctx;

fn trace_value_to_js(v: &TraceValue) -> JsValue {
    match v {
        TraceValue::Str(s) => JsValue::from(js_string!(s.as_str())),
        TraceValue::Num(n) => JsValue::from(*n),
        TraceValue::Bool(b) => JsValue::from(*b),
        TraceValue::Null => JsValue::null(),
    }
}

fn offset_object(ctx: &mut Context, x: f64, y: f64) -> JsResult<JsValue> {
    let obj = JsObject::with_object_proto(ctx.intrinsics());
    obj.create_data_property(js_string!("x"), JsValue::from(x), ctx)?;
    obj.create_data_property(js_string!("y"), JsValue::from(y), ctx)?;
    Ok(obj.into())
}

fn dev_node_to_js(node: DevNodeData, ctx: &mut Context) -> JsResult<JsValue> {
    let obj = JsObject::with_object_proto(ctx.intrinsics());

    obj.create_data_property(
        js_string!("id"),
        JsValue::from(node.id.as_u64() as f64),
        ctx,
    )?;
    obj.create_data_property(
        js_string!("name"),
        JsValue::from(js_string!(node.name)),
        ctx,
    )?;
    obj.create_data_property(
        js_string!("label"),
        JsValue::from(js_string!(node.label.as_str())),
        ctx,
    )?;

    // props
    let props = JsObject::with_object_proto(ctx.intrinsics());
    for (k, v) in &node.props {
        props.create_data_property(js_string!(*k), trace_value_to_js(v), ctx)?;
    }
    obj.create_data_property(js_string!("props"), JsValue::from(props), ctx)?;

    // layout: { relative, absolute, width, height, extra }
    let layout = JsObject::with_object_proto(ctx.intrinsics());
    layout.create_data_property(
        js_string!("relative"),
        offset_object(ctx, node.relative.0, node.relative.1)?,
        ctx,
    )?;
    layout.create_data_property(
        js_string!("absolute"),
        offset_object(ctx, node.absolute.0, node.absolute.1)?,
        ctx,
    )?;
    layout.create_data_property(js_string!("width"), JsValue::from(node.size.0), ctx)?;
    layout.create_data_property(js_string!("height"), JsValue::from(node.size.1), ctx)?;
    if !node.layout_extra.is_empty() {
        let extra = JsObject::with_object_proto(ctx.intrinsics());
        for (k, v) in &node.layout_extra {
            extra.create_data_property(js_string!(*k), trace_value_to_js(v), ctx)?;
        }
        layout.create_data_property(js_string!("extra"), JsValue::from(extra), ctx)?;
    }
    obj.create_data_property(js_string!("layout"), JsValue::from(layout), ctx)?;

    // queryKey
    if let Some(keys) = &node.query_key {
        let arr = JsArray::new(ctx)?;
        for k in keys {
            arr.push(JsValue::from(js_string!(k.as_str())), ctx)?;
        }
        obj.create_data_property(js_string!("queryKey"), JsValue::from(arr), ctx)?;
    }

    // children: Array<{ id }>
    let children = JsArray::new(ctx)?;
    for child_id in &node.children {
        let child_obj = JsObject::with_object_proto(ctx.intrinsics());
        child_obj.create_data_property(
            js_string!("id"),
            JsValue::from(child_id.as_u64() as f64),
            ctx,
        )?;
        children.push(JsValue::from(child_obj), ctx)?;
    }
    obj.create_data_property(js_string!("children"), JsValue::from(children), ctx)?;

    Ok(obj.into())
}

pub fn tur_dev_tool_element_tree(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let tree = js_ctx.element_tree.borrow();
    let Some(root_id) = tree.root_element_id() else {
        return Ok(JsValue::null());
    };
    match tree.dev_tool_node(root_id.into()) {
        Some(node) => dev_node_to_js(node, ctx),
        None => Ok(JsValue::null()),
    }
}

pub fn tur_dev_tool_get_element(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let raw = args.get_or_undefined(1);
    let id = raw
        .as_number()
        .ok_or_else(|| {
            boa_engine::JsError::from(
                boa_engine::JsNativeError::typ()
                    .with_message("getElement: expected a numeric id as the second argument"),
            )
        })
        .map(|n| crate::core::element::NodeId::new(n as u64))?;
    let tree = js_ctx.element_tree.borrow();
    match tree.dev_tool_node(id) {
        Some(node) => dev_node_to_js(node, ctx),
        None => Ok(JsValue::null()),
    }
}

/// `turDevTool.reactiveStats()` — `{ subscribers, edges }` over the shared
/// reactive subscriber graph (elements + fragments).
pub fn tur_dev_tool_reactive_stats(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let (subscribers, edges) = js_ctx.reactive_subscriber_stats();
    let obj = JsObject::with_object_proto(ctx.intrinsics());
    obj.create_data_property(
        js_string!("subscribers"),
        JsValue::from(subscribers as f64),
        ctx,
    )?;
    obj.create_data_property(js_string!("edges"), JsValue::from(edges as f64), ctx)?;
    Ok(obj.into())
}

/// `turDevTool.frameStats()` — per-instance render-performance probe (see
/// `core::app::frame_stats`). Worker-side counters are always-on; host-side
/// render-commit timings arrive via `WorkerMsg::FrameTiming` only after
/// `setHostFrameTiming(true)`.
///
/// Returns `{ flushes, paintedFrames, totals, last, lastHost }` where
/// `last` is the most recent painted frame's worker timing (or `null`)
/// and `lastHost` the most recent host render-commit timing (or `null`).
pub fn tur_dev_tool_frame_stats(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let stats = &js_ctx.frame_stats;

    fn timing_object(t: &FrameTiming, ctx: &mut Context) -> JsResult<JsValue> {
        let obj = JsObject::with_object_proto(ctx.intrinsics());
        let fields: &[(&str, f64)] = &[
            ("frame", t.frame_id as f64),
            ("nodesWalked", t.nodes_walked as f64),
            ("opsRecorded", t.ops_recorded as f64),
            ("commands", t.commands_emitted as f64),
            ("batchBytes", t.batch_bytes as f64),
            ("dirtyLayoutNodes", t.dirty_layout_nodes as f64),
            ("flushUs", t.flush_us as f64),
            ("layoutUs", t.layout_us as f64),
            ("recordWalkUs", t.record_walk_us as f64),
            ("batchPostUs", t.batch_post_us as f64),
        ];
        for (k, v) in fields {
            obj.create_data_property(js_string!(*k), JsValue::from(*v), ctx)?;
        }
        Ok(obj.into())
    }

    fn host_timing_object(t: &HostFrameTiming, ctx: &mut Context) -> JsResult<JsValue> {
        let obj = JsObject::with_object_proto(ctx.intrinsics());
        for (k, v) in [
            ("frame", t.frame_id as f64),
            ("applyUs", t.apply_us as f64),
            ("presentUs", t.present_us as f64),
        ] {
            obj.create_data_property(js_string!(k), JsValue::from(v), ctx)?;
        }
        Ok(obj.into())
    }

    let obj = JsObject::with_object_proto(ctx.intrinsics());
    obj.create_data_property(
        js_string!("flushes"),
        JsValue::from(stats.flushes.get() as f64),
        ctx,
    )?;
    obj.create_data_property(
        js_string!("paintedFrames"),
        JsValue::from(stats.painted_frames.get() as f64),
        ctx,
    )?;

    let totals = JsObject::with_object_proto(ctx.intrinsics());
    for (k, v) in [
        ("flushUs", stats.total_flush_us.get() as f64),
        ("nodesWalked", stats.total_nodes_walked.get() as f64),
        ("opsRecorded", stats.total_ops_recorded.get() as f64),
    ] {
        totals.create_data_property(js_string!(k), JsValue::from(v), ctx)?;
    }
    obj.create_data_property(js_string!("totals"), JsValue::from(totals), ctx)?;

    let last = stats.last.borrow().clone();
    match last {
        Some(t) => {
            let o = timing_object(&t, ctx)?;
            obj.create_data_property(js_string!("last"), o, ctx)?;
        }
        None => {
            obj.create_data_property(js_string!("last"), JsValue::null(), ctx)?;
        }
    }
    let last_host = stats.last_host.borrow().clone();
    match last_host {
        Some(t) => {
            let o = host_timing_object(&t, ctx)?;
            obj.create_data_property(js_string!("lastHost"), o, ctx)?;
        }
        None => {
            obj.create_data_property(js_string!("lastHost"), JsValue::null(), ctx)?;
        }
    }
    obj.create_data_property(
        js_string!("hostTimingEnabled"),
        JsValue::from(stats.host_timing_enabled.get()),
        ctx,
    )?;
    Ok(obj.into())
}

/// `turDevTool.setHostFrameTiming(enabled)` — toggle host-side render-commit
/// timing collection. Sets the worker-side flag (reflected in
/// `frameStats()`) and ships the toggle to main (`HostMsg::FrameTimingEnabled`),
/// where `HostBackend` gates its per-frame `WorkerMsg::FrameTiming` push-back.
pub fn tur_dev_tool_set_host_frame_timing(
    _this: &JsValue,
    args: &[JsValue],
    _ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let enabled = args.get_or_undefined(1).as_boolean().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message("setHostFrameTiming: expected a boolean as the first argument"),
        )
    })?;
    js_ctx.frame_stats.host_timing_enabled.set(enabled);
    let _ = js_ctx
        .host_tx
        .unbounded_send(crate::core::app::HostMsg::FrameTimingEnabled(enabled));
    Ok(JsValue::undefined())
}
