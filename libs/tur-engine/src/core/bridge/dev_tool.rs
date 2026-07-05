//! Dev-tool bridge: `__tur._dev_tool_element_tree` /
//! `__tur._dev_tool_get_element`, plus the public `turDevTool` global that
//! wraps them.
//!
//! The public JS API is `turDevTool.elementTree()` /
//! `turDevTool.getElement(id)`. Those are thin wrappers (registered as a
//! small JS snippet at init) that forward to the underscore-prefixed
//! natives on `__tur`, auto-passing `__tur.__ctx` — matching the layering
//! every other `__tur.*` API uses.

use boa_engine::object::builtins::JsArray;
use boa_engine::object::JsObject;
use boa_engine::{js_string, Context, JsArgs, JsResult, JsValue};

use crate::core::bridge::helpers::extract_ctx;
use crate::core::elements::{DevNodeData, TraceValue};

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

    obj.create_data_property(js_string!("id"), JsValue::from(node.id.as_u64() as f64), ctx)?;
    obj.create_data_property(js_string!("name"), JsValue::from(js_string!(node.name)), ctx)?;
    obj.create_data_property(js_string!("label"), JsValue::from(js_string!(node.label.as_str())), ctx)?;

    // props
    let props = JsObject::with_object_proto(ctx.intrinsics());
    for (k, v) in &node.props {
        props.create_data_property(js_string!(*k), trace_value_to_js(v), ctx)?;
    }
    obj.create_data_property(js_string!("props"), JsValue::from(props), ctx)?;

    // layout: { relative, absolute, width, height, extra }
    let layout = JsObject::with_object_proto(ctx.intrinsics());
    layout.create_data_property(js_string!("relative"), offset_object(ctx, node.relative.0, node.relative.1)?, ctx)?;
    layout.create_data_property(js_string!("absolute"), offset_object(ctx, node.absolute.0, node.absolute.1)?, ctx)?;
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
        child_obj.create_data_property(js_string!("id"), JsValue::from(child_id.as_u64() as f64), ctx)?;
        children.push(JsValue::from(child_obj), ctx)?;
    }
    obj.create_data_property(js_string!("children"), JsValue::from(children), ctx)?;

    Ok(obj.into())
}

pub(crate) fn tur_dev_tool_element_tree(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let tree = js_ctx.element_tree.borrow();
    let Some(root_id) = tree.root_element_id() else {
        return Ok(JsValue::null());
    };
    match tree.dev_tool_node(root_id.into()) {
        Some(node) => dev_node_to_js(node, ctx),
        None => Ok(JsValue::null()),
    }
}

pub(crate) fn tur_dev_tool_get_element(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
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
