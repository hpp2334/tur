use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::{Rc, Weak};

use boa_engine::object::JsObject;
use boa_engine::{Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};
use tur_shared::Constraints;

use crate::core::bridge::BoaOpaque;
use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementNode, ElementTree};
use crate::core::event::{AppEvent, AppPointerEvent, EventKind, RawAppEvent};
use crate::core::render::Renderer;
use crate::elements::{
    ContainerElement, FlexElement, FlexItemElement, PointerInteractElement, PositionedElement,
    StackElement, TextElement,
};

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
    pub(crate) id: ElementNodeId,
}

struct GestureArena {
    pointer_down_target: Option<ElementNodeId>,
}

impl GestureArena {
    fn new() -> Self {
        Self {
            pointer_down_target: None,
        }
    }

    fn process_raw_event(
        &mut self,
        raw: &RawAppEvent,
        tree: &ElementTree,
    ) -> (AppEvent, Option<Vec<AppEvent>>) {
        match raw {
            RawAppEvent::PointerDown { position } => {
                let path = tree.hit_test_path(*position);
                self.pointer_down_target = path.first().copied();
                (
                    AppEvent::PointerDown(AppPointerEvent {
                        position: *position,
                    }),
                    None,
                )
            }
            RawAppEvent::PointerUp { position } => {
                let path = tree.hit_test_path(*position);
                let down_target = self.pointer_down_target.take();
                let gesture = match down_target {
                    Some(id) if path.contains(&id) => {
                        Some(vec![AppEvent::Click(AppPointerEvent {
                            position: *position,
                        })])
                    }
                    _ => None,
                };
                (
                    AppEvent::PointerUp(AppPointerEvent {
                        position: *position,
                    }),
                    gesture,
                )
            }
        }
    }
}

pub struct TurAppContext {
    element_tree: Rc<RefCell<ElementTree>>,
    renderer: RefCell<Box<dyn Renderer>>,
    size: Cell<(f64, f64)>,
    next_id: Cell<u64>,
    handles: RefCell<HashMap<ElementNodeId, BoaOpaque<TurNodeHandle>>>,
    event_handlers: RefCell<HashMap<(ElementNodeId, EventKind), JsObject>>,
    gesture_arena: RefCell<GestureArena>,
}

impl fmt::Debug for TurAppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurAppContext")
            .field("element_tree", &self.element_tree)
            .field("size", &self.size)
            .field("next_id", &self.next_id)
            .field("handles", &self.handles)
            .finish_non_exhaustive()
    }
}

impl TurAppContext {
    pub fn new(renderer: Box<dyn Renderer>) -> Self {
        Self {
            element_tree: Rc::new(RefCell::new(ElementTree::new())),
            renderer: RefCell::new(renderer),
            size: Cell::new((400.0, 600.0)),
            next_id: Cell::new(1),
            handles: RefCell::new(HashMap::new()),
            event_handlers: RefCell::new(HashMap::new()),
            gesture_arena: RefCell::new(GestureArena::new()),
        }
    }

    pub fn element_tree(&self) -> &RefCell<ElementTree> {
        &self.element_tree
    }

    pub fn element_tree_rc(&self) -> Rc<RefCell<ElementTree>> {
        Rc::clone(&self.element_tree)
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
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        };

        {
            let mut tree = self.element_tree.borrow_mut();
            let layout_size = tree.compute_layout(&constraints);
            tracing::debug!("layout: {:?}", layout_size);
        }

        let mut renderer = self.renderer.borrow_mut();
        let tree = self.element_tree.borrow();
        renderer.render(&tree);
    }

    fn alloc_id(&self) -> ElementNodeId {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        ElementNodeId::new(id)
    }

    fn get_or_create_handle(
        &self,
        id: ElementNodeId,
        context: &mut Context,
    ) -> BoaOpaque<TurNodeHandle> {
        if let Some(opaque) = self.handles.borrow().get(&id) {
            return opaque.clone();
        }
        let opaque = BoaOpaque::new(TurNodeHandle { id }, context);
        self.handles.borrow_mut().insert(id, opaque.clone());
        opaque
    }

    pub fn has_event_handler(&self, id: ElementNodeId, kind: EventKind) -> bool {
        self.event_handlers.borrow().contains_key(&(id, kind))
    }

    pub fn dispatch_raw_event(&self, raw: &RawAppEvent, context: &mut Context) {
        let (direct_event, gesture_events) = {
            let tree = self.element_tree.borrow();
            self.gesture_arena
                .borrow_mut()
                .process_raw_event(raw, &tree)
        };

        self.invoke_handlers(&direct_event, context);

        if let Some(events) = gesture_events {
            for event in events {
                self.invoke_handlers(&event, context);
            }
        }
    }

    fn invoke_handlers(&self, event: &AppEvent, context: &mut Context) {
        let (kind, position) = match event {
            AppEvent::PointerDown(e) => (EventKind::PointerDown, e.position),
            AppEvent::PointerUp(e) => (EventKind::PointerUp, e.position),
            AppEvent::Click(e) => (EventKind::Click, e.position),
        };

        let tree = self.element_tree.borrow();
        let path = tree.hit_test_path(position);
        drop(tree);

        let callbacks: Vec<JsObject> = {
            let handlers = self.event_handlers.borrow();
            path.iter()
                .filter_map(|id| handlers.get(&(*id, kind)).cloned())
                .collect()
        };

        for callback in callbacks {
            let _ = callback.call(&JsValue::undefined(), &[], context);
        }
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

fn extract_node_id(args: &[JsValue], idx: usize) -> JsResult<ElementNodeId> {
    let obj = args.get_or_undefined(idx).as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected TurNodeHandle"))
    })?;
    let handle = BoaOpaque::<TurNodeHandle>::wrap(&obj).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected TurNodeHandle"))
    })?;
    Ok(handle.id)
}

fn create_element(
    args: &[JsValue],
    context: &mut Context,
    element: AnyElement,
) -> JsResult<JsValue> {
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
    let id = ctx.alloc_id();
    let node = ElementNode::new(id, element);
    ctx.element_tree.borrow_mut().insert(node);

    let obj = ctx.get_or_create_handle(id, context);
    Ok(obj.object().clone().into())
}

pub(crate) fn tur_create_flex(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createFlex()");
    create_element(args, context, AnyElement::new(FlexElement::new()))
}

pub(crate) fn tur_create_flex_item(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createFlexItem()");
    create_element(args, context, AnyElement::new(FlexItemElement::new()))
}

pub(crate) fn tur_create_stack(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createStack()");
    create_element(args, context, AnyElement::new(StackElement::new()))
}

pub(crate) fn tur_create_positioned(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createPositioned()");
    create_element(args, context, AnyElement::new(PositionedElement::new()))
}

pub(crate) fn tur_create_container(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createContainer()");
    create_element(args, context, AnyElement::new(ContainerElement::new()))
}

pub(crate) fn tur_create_text(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createText()");
    create_element(args, context, AnyElement::new(TextElement::new()))
}

pub(crate) fn tur_create_pointer_interact(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createPointerInteract()");
    create_element(
        args,
        context,
        AnyElement::new(PointerInteractElement::new()),
    )
}

pub(crate) fn tur_create_root(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createRoot()");
    create_element(args, context, AnyElement::new(FlexElement::new()))
}

pub(crate) fn tur_set_attribute(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow();
    let node_id = extract_node_id(args, 1)?;
    let key = args.get_or_undefined(2).to_string(context)?;

    let value = args.get_or_undefined(3).clone();

    tracing::trace!(
        "tur_setAttribute({}, {}, ...)",
        node_id,
        key.to_std_string_escaped()
    );

    if key == "queryKey" {
        if let Some(obj) = value.as_object() {
            if let Ok(arr) = boa_engine::object::builtins::JsArray::from_object(obj.clone()) {
                let len = arr.length(context).unwrap_or(0);
                let mut keys = Vec::with_capacity(len as usize);
                for i in 0..len {
                    if let Ok(val) = arr.at(i as i64, context) {
                        if let Some(s) = val.as_string() {
                            keys.push(s.to_std_string_escaped());
                        }
                    }
                }
                if let Some(node) = ctx.element_tree.borrow_mut().get_mut(node_id) {
                    node.query_key = if keys.is_empty() { None } else { Some(keys) };
                }
            }
        }
        return Ok(JsValue::undefined());
    }

    if let Some(node) = ctx.element_tree.borrow().get(node_id) {
        if let Some(ref element) = node.element {
            if element.type_name() == "tur_pointer_interact" && key == "onClick" {
                let event_kind = EventKind::Click;
                if let Some(obj) = value.as_object() {
                    if obj.is_callable() {
                        ctx.event_handlers
                            .borrow_mut()
                            .insert((node_id, event_kind), obj.clone());
                    }
                } else if value.is_null() || value.is_undefined() {
                    ctx.event_handlers
                        .borrow_mut()
                        .remove(&(node_id, event_kind));
                }
                return Ok(JsValue::undefined());
            }
        }
    }

    if let Some(node) = ctx.element_tree.borrow_mut().get_mut(node_id) {
        if let Some(ref mut element) = node.element {
            element.set_prop(context, &key, &value);
        }
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

    ctx.element_tree
        .borrow_mut()
        .append_child(parent_id, child_id);

    tracing::trace!("tur_appendChild({}, {})", parent_id, child_id);
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

    ctx.element_tree
        .borrow_mut()
        .remove_child(parent_id, child_id);

    tracing::trace!("tur_removeChild({}, {})", parent_id, child_id);
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

    ctx.element_tree
        .borrow_mut()
        .insert_before(parent_id, child_id, ref_id);

    tracing::trace!("tur_insertBefore({}, {}, {})", parent_id, child_id, ref_id);
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
    match ctx.element_tree.borrow().parent_of(node_id) {
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
    match ctx.element_tree.borrow().first_child_of(node_id) {
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
    match ctx.element_tree.borrow().next_sibling_of(node_id) {
        Some(sibling_id) => {
            let obj = ctx.get_or_create_handle(sibling_id, context);
            Ok(obj.object().clone().into())
        }
        None => Ok(JsValue::null()),
    }
}
