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
use boa_engine::{Context, JsArgs, JsResult, JsValue, js_string};

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

/// Node id → JS: `{ root, node }` object (numbers). Round-trips through
/// [`node_id_from_js`].
fn node_id_to_js(id: crate::core::element::NodeId, ctx: &mut Context) -> JsResult<JsValue> {
    let obj = JsObject::with_object_proto(ctx.intrinsics());
    obj.create_data_property(
        js_string!("root"),
        JsValue::from(id.root().as_u32() as f64),
        ctx,
    )?;
    obj.create_data_property(js_string!("node"), JsValue::from(id.node() as f64), ctx)?;
    Ok(obj.into())
}

/// JS `{ root, node }` object → node id.
fn node_id_from_js(raw: &JsValue, ctx: &mut Context) -> Option<crate::core::element::NodeId> {
    let obj = raw.as_object()?;
    let root = obj.get(js_string!("root"), ctx).ok()?;
    let node = obj.get(js_string!("node"), ctx).ok()?;
    Some(crate::core::element::NodeId::new(
        crate::core::element::ViewRootId::new(root.as_number()? as u32),
        node.as_number()? as u64,
    ))
}

fn dev_node_to_js(node: DevNodeData, ctx: &mut Context) -> JsResult<JsValue> {
    let obj = JsObject::with_object_proto(ctx.intrinsics());

    obj.create_data_property(js_string!("id"), node_id_to_js(node.id, ctx)?, ctx)?;
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
        child_obj.create_data_property(js_string!("id"), node_id_to_js(*child_id, ctx)?, ctx)?;
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
    // First setup root's tree (the playground is single-root).
    let Some(tree) = js_ctx
        .view_roots
        .borrow()
        .setup_roots()
        .into_iter()
        .next()
        .map(|(_, t)| t)
    else {
        return Ok(JsValue::null());
    };
    let tree = tree.borrow();
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
    let Some(id) = node_id_from_js(raw, ctx) else {
        return Err(boa_engine::JsError::from(
            boa_engine::JsNativeError::typ().with_message(
                "getElement: expected an id object `{ root, node }` as the second argument",
            ),
        ));
    };
    let Some(tree) = js_ctx.tree_containing(id) else {
        return Ok(JsValue::null());
    };
    match tree.dev_tool_node(id) {
        Some(node) => dev_node_to_js(node, ctx),
        None => Ok(JsValue::null()),
    }
}
