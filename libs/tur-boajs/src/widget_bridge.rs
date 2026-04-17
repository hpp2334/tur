use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;

use boa_engine::{Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};
use tracing;
use tur_widget::{PropValue, WidgetKind, WidgetNode, WidgetNodeId, WidgetTree};

use crate::BoaOpaque;

#[derive(Debug, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurNodeHandle {
    pub(crate) id: WidgetNodeId,
}

#[derive(Debug, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurAppContext {
    tree: Rc<RefCell<WidgetTree>>,
    next_id: Cell<u64>,
    handles: RefCell<HashMap<u64, BoaOpaque<TurNodeHandle>>>,
}

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
            handles: RefCell::new(HashMap::new()),
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

    fn get_or_create_handle(
        &self,
        id: WidgetNodeId,
        context: &mut Context,
    ) -> BoaOpaque<TurNodeHandle> {
        let key = id.as_u64();
        if let Some(opaque) = self.handles.borrow().get(&key) {
            return opaque.clone();
        }
        let opaque = BoaOpaque::new(TurNodeHandle { id }, context);
        self.handles.borrow_mut().insert(key, opaque.clone());
        opaque
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

fn extract_node_id(args: &[JsValue], idx: usize) -> JsResult<WidgetNodeId> {
    let obj = args.get_or_undefined(idx).as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected TurNodeHandle"))
    })?;
    let handle = BoaOpaque::<TurNodeHandle>::wrap(&obj).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected TurNodeHandle"))
    })?;
    Ok(handle.id)
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
    let obj = ctx.get_or_create_handle(id, context);
    Ok(obj.object().clone().into())
}

pub(crate) fn tur_create_root(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let id = ctx.alloc_id();
    let node = WidgetNode::new(id, WidgetKind::Column);
    ctx.tree.borrow_mut().insert(node);

    tracing::trace!("tur_createRoot() -> {}", id.as_u64());
    let obj = ctx.get_or_create_handle(id, context);
    Ok(obj.object().clone().into())
}

pub(crate) fn tur_set_attribute(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let node_id = extract_node_id(args, 1)?;
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

    if let Some(node) = ctx.tree.borrow_mut().get_mut(node_id) {
        node.set_prop(key, prop_value);
    }

    Ok(JsValue::undefined())
}

pub(crate) fn tur_append_child(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let parent_id = extract_node_id(args, 1)?;
    let child_id = extract_node_id(args, 2)?;

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
    _context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let parent_id = extract_node_id(args, 1)?;
    let child_id = extract_node_id(args, 2)?;

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
    _context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let parent_id = extract_node_id(args, 1)?;
    let child_id = extract_node_id(args, 2)?;
    let ref_id = extract_node_id(args, 3)?;

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
    let node_id = extract_node_id(args, 1)?;
    match ctx.tree.borrow().parent_of(node_id) {
        Some(parent_id) => {
            let obj = ctx.get_or_create_handle(parent_id, context);
            Ok(obj.object().clone().into())
        }
        None => Ok(JsValue::null()),
    }
}

pub(crate) fn tur_get_first_child(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let node_id = extract_node_id(args, 1)?;
    match ctx.tree.borrow().first_child_of(node_id) {
        Some(child_id) => {
            let obj = ctx.get_or_create_handle(child_id, context);
            Ok(obj.object().clone().into())
        }
        None => Ok(JsValue::null()),
    }
}

pub(crate) fn tur_get_next_sibling(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    extract_ctx!(args, ctx);
    let node_id = extract_node_id(args, 1)?;
    match ctx.tree.borrow().next_sibling_of(node_id) {
        Some(sibling_id) => {
            let obj = ctx.get_or_create_handle(sibling_id, context);
            Ok(obj.object().clone().into())
        }
        None => Ok(JsValue::null()),
    }
}
