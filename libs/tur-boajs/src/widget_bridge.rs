use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::{Rc, Weak};
use std::str::FromStr;

use boa_engine::{Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};
use tracing;
use tur_layout::LayoutTree;
use tur_render_tree::{RenderTree, Renderer};
use tur_shared::Constraints;
use tur_widget::{PropValue, WidgetKind, WidgetNode, WidgetNodeId, WidgetTree};

use crate::BoaOpaque;

#[derive(Clone, Debug, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct WeakAppContext {
    inner: Weak<RefCell<TurAppContext>>,
}

impl WeakAppContext {
    pub fn new(rc: &Rc<RefCell<TurAppContext>>) -> Self {
        Self {
            inner: Rc::downgrade(rc),
        }
    }

    pub fn upgrade(&self) -> Option<Rc<RefCell<TurAppContext>>> {
        self.inner.upgrade()
    }
}

#[derive(Debug, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurNodeHandle {
    pub(crate) id: WidgetNodeId,
}

pub struct TurAppContext {
    tree: Rc<RefCell<WidgetTree>>,
    layout_tree: RefCell<LayoutTree>,
    render_tree: RefCell<RenderTree>,
    renderer: RefCell<Box<dyn Renderer>>,
    size: Cell<(f64, f64)>,
    next_id: Cell<u64>,
    handles: RefCell<HashMap<u64, BoaOpaque<TurNodeHandle>>>,
}

impl fmt::Debug for TurAppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurAppContext")
            .field("tree", &self.tree)
            .field("layout_tree", &self.layout_tree)
            .field("render_tree", &self.render_tree)
            .field("size", &self.size)
            .field("next_id", &self.next_id)
            .field("handles", &self.handles)
            .finish_non_exhaustive()
    }
}

impl TurAppContext {
    pub fn new(renderer: Box<dyn Renderer>) -> Self {
        Self {
            tree: Rc::new(RefCell::new(WidgetTree::new())),
            layout_tree: RefCell::new(LayoutTree::default()),
            render_tree: RefCell::new(RenderTree::default()),
            renderer: RefCell::new(renderer),
            size: Cell::new((400.0, 600.0)),
            next_id: Cell::new(1),
            handles: RefCell::new(HashMap::new()),
        }
    }

    pub fn tree(&self) -> &RefCell<WidgetTree> {
        &self.tree
    }

    pub fn layout_tree(&self) -> &RefCell<LayoutTree> {
        &self.layout_tree
    }

    pub fn render_tree(&self) -> &RefCell<RenderTree> {
        &self.render_tree
    }

    pub fn renderer(&self) -> &RefCell<Box<dyn Renderer>> {
        &self.renderer
    }

    pub fn set_size(&self, width: f64, height: f64) {
        self.size.set((width, height));
    }

    pub fn render(&self) {
        let (width, height) = self.size.get();
        let constraints = Constraints {
            min_width: 0.0,
            max_width: width,
            min_height: 0.0,
            max_height: height,
        };

        let tree_guard = self.tree.borrow();
        let mut layout_tree = self.layout_tree.borrow_mut();
        let mut render_tree = self.render_tree.borrow_mut();

        layout_tree.rebuild_from_widget_tree(&tree_guard);
        let result = layout_tree.compute_layout(&constraints);
        tracing::debug!("layout: {:?}", result.size);

        render_tree.rebuild_from_layout_tree(&layout_tree, &tree_guard);

        let mut renderer = self.renderer.borrow_mut();
        renderer.render(&render_tree);
        tracing::debug!("rendered: {:?}", result.size);
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

fn extract_ctx(args: &[JsValue]) -> JsResult<Rc<RefCell<TurAppContext>>> {
    let obj = args.get_or_undefined(0).as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected TurAppContext as first argument"))
    })?;
    let weak = BoaOpaque::<WeakAppContext>::wrap(&obj).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected TurAppContext as first argument"))
    })?;
    weak.upgrade().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("TurAppContext has been dropped"))
    })
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
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
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
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
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
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
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
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
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
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
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
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
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
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
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
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
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
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
    let node_id = extract_node_id(args, 1)?;
    match ctx.tree.borrow().next_sibling_of(node_id) {
        Some(sibling_id) => {
            let obj = ctx.get_or_create_handle(sibling_id, context);
            Ok(obj.object().clone().into())
        }
        None => Ok(JsValue::null()),
    }
}
