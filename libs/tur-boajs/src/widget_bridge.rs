use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::str::FromStr;

use boa_engine::{Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{empty_trace, Finalize};
use tracing;
use tur_widget::{PropValue, WidgetKind, WidgetNode, WidgetNodeId, WidgetTree};

use crate::BoaOpaque;

#[derive(Debug)]
pub struct TurAppContext {
    tree: Rc<RefCell<WidgetTree>>,
    next_id: Cell<u64>,
}

impl Finalize for TurAppContext {}

unsafe impl boa_gc::Trace for TurAppContext {
    empty_trace!();
}

impl JsData for TurAppContext {}

impl Default for TurAppContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TurAppContext {
    pub fn new() -> Self {
        Self {
            tree: Rc::new(RefCell::new(WidgetTree::new())),
            next_id: Cell::new(1),
        }
    }

    pub fn tree(&self) -> &RefCell<WidgetTree> {
        &self.tree
    }

    pub fn tree_rc(&self) -> &Rc<RefCell<WidgetTree>> {
        &self.tree
    }

    fn alloc_id(&self) -> WidgetNodeId {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        WidgetNodeId::new(id)
    }
}

macro_rules! extract_ctx {
    ($args:expr, $ctx:ident) => {
        let __obj = $args.get_or_undefined(0).as_object().ok_or_else(|| {
            JsError::from(
                JsNativeError::typ().with_message("expected TurAppContext as first argument"),
            )
        })?;
        let $ctx = BoaOpaque::<TurAppContext>::wrap(&__obj).ok_or_else(|| {
            JsError::from(
                JsNativeError::typ().with_message("expected TurAppContext as first argument"),
            )
        })?;
    };
}

pub(crate) fn tur_create_app_context(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = TurAppContext::new();
    let opaque = BoaOpaque::new(ctx, context);
    Ok(opaque.object().clone().into())
}

pub(crate) fn tur_create_element(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let kind_str = args
        .get_or_undefined(1)
        .to_string(context)?
        .to_std_string_escaped();

    let kind = WidgetKind::from_str(&kind_str).unwrap_or_else(|_| {
        tracing::warn!("unknown widget type: {kind_str}, falling back to Container");
        WidgetKind::Container
    });

    let id = ctx.alloc_id();
    let node = WidgetNode::new(id, kind);
    ctx.tree.borrow_mut().insert(node);

    tracing::trace!("tur_createElement({kind_str}) -> {}", id.as_u64());
    Ok(JsValue::from(id.as_u64() as f64))
}

pub(crate) fn tur_create_root(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let id = ctx.alloc_id();
    let node = WidgetNode::new(id, WidgetKind::Column);
    ctx.tree.borrow_mut().insert(node);

    tracing::trace!("tur_createRoot() -> {}", id.as_u64());
    Ok(JsValue::from(id.as_u64() as f64))
}

pub(crate) fn tur_set_attribute(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = WidgetNodeId::new(args.get_or_undefined(1).to_number(context)? as u64);
    let key = args
        .get_or_undefined(2)
        .to_string(context)?
        .to_std_string_escaped();
    let value = args.get_or_undefined(3).clone();

    let prop_value = if let Some(s) = value.as_string() {
        PropValue::String(s.to_std_string_escaped())
    } else if let Some(n) = value.as_number() {
        PropValue::Number(n)
    } else if let Some(b) = value.as_boolean() {
        PropValue::Bool(b)
    } else if let Some(b) = value.as_bigint() {
        let n: i64 = b.to_string().parse().unwrap_or(0);
        PropValue::Number(n as f64)
    } else {
        PropValue::String(
            value
                .to_string(context)
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default(),
        )
    };

    tracing::trace!("tur_setAttribute({}, {key}, ...)", node_id.as_u64());

    extract_ctx!(args, ctx);

    if let Some(node) = ctx.tree.borrow_mut().get_mut(node_id) {
        node.set_prop(key, prop_value);
    }

    Ok(JsValue::undefined())
}

pub(crate) fn tur_append_child(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let parent_id = WidgetNodeId::new(args.get_or_undefined(1).to_number(context)? as u64);
    let child_id = WidgetNodeId::new(args.get_or_undefined(2).to_number(context)? as u64);

    ctx.tree.borrow_mut().append_child(parent_id, child_id);

    tracing::trace!(
        "tur_appendChild({}, {})",
        parent_id.as_u64(),
        child_id.as_u64()
    );
    Ok(JsValue::undefined())
}

pub(crate) fn tur_remove_child(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let parent_id = WidgetNodeId::new(args.get_or_undefined(1).to_number(context)? as u64);
    let child_id = WidgetNodeId::new(args.get_or_undefined(2).to_number(context)? as u64);

    ctx.tree.borrow_mut().remove_child(parent_id, child_id);

    tracing::trace!(
        "tur_removeChild({}, {})",
        parent_id.as_u64(),
        child_id.as_u64()
    );
    Ok(JsValue::undefined())
}

pub(crate) fn tur_insert_before(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let parent_id = WidgetNodeId::new(args.get_or_undefined(1).to_number(context)? as u64);
    let child_id = WidgetNodeId::new(args.get_or_undefined(2).to_number(context)? as u64);
    let ref_id = WidgetNodeId::new(args.get_or_undefined(3).to_number(context)? as u64);

    ctx.tree
        .borrow_mut()
        .insert_before(parent_id, child_id, ref_id);

    tracing::trace!(
        "tur_insertBefore({}, {}, {})",
        parent_id.as_u64(),
        child_id.as_u64(),
        ref_id.as_u64()
    );
    Ok(JsValue::undefined())
}

pub(crate) fn tur_get_parent(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let node_id = WidgetNodeId::new(args.get_or_undefined(1).to_number(context)? as u64);
    match ctx.tree.borrow().parent_of(node_id) {
        Some(parent_id) => Ok(JsValue::from(parent_id.as_u64() as f64)),
        None => Ok(JsValue::null()),
    }
}

pub(crate) fn tur_get_first_child(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let node_id = WidgetNodeId::new(args.get_or_undefined(1).to_number(context)? as u64);
    match ctx.tree.borrow().first_child_of(node_id) {
        Some(child_id) => Ok(JsValue::from(child_id.as_u64() as f64)),
        None => Ok(JsValue::null()),
    }
}

pub(crate) fn tur_get_next_sibling(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let node_id = WidgetNodeId::new(args.get_or_undefined(1).to_number(context)? as u64);
    match ctx.tree.borrow().next_sibling_of(node_id) {
        Some(sibling_id) => Ok(JsValue::from(sibling_id.as_u64() as f64)),
        None => Ok(JsValue::null()),
    }
}
