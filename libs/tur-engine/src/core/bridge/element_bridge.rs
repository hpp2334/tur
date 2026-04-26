use std::cell::RefCell;
use std::rc::{Rc, Weak};

use boa_engine::{Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::app::TurAppInternal;
use crate::core::bridge::BoaOpaque;
use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementNode};
use crate::core::event::AppEvent;
use crate::core::gesture::ComposedGestureEventKind;
use crate::elements::{
    ContainerElement, FlexElement, FlexItemElement, PointerInteractElement, PositionedElement,
    StackElement, TextElement,
};

#[derive(Clone, Debug, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct WeakAppContext {
    inner: Weak<RefCell<TurAppInternal>>,
}

impl WeakAppContext {
    pub fn new(rc: &Rc<RefCell<TurAppInternal>>) -> Self {
        Self {
            inner: Rc::downgrade(rc),
        }
    }

    pub fn upgrade(&self) -> Option<Rc<RefCell<TurAppInternal>>> {
        self.inner.upgrade()
    }
}

#[derive(Debug, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurNodeHandle {
    pub(crate) id: ElementNodeId,
}

fn extract_ctx(args: &[JsValue]) -> JsResult<Rc<RefCell<TurAppInternal>>> {
    let obj = args.get_or_undefined(0).as_object().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("expected TurAppInternal as first argument"),
        )
    })?;
    let weak = BoaOpaque::<WeakAppContext>::wrap(&obj).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("expected TurAppInternal as first argument"),
        )
    })?;
    weak.upgrade().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("TurAppInternal has been dropped"))
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
        ctx.push_event(AppEvent::RequestDraw);
        return Ok(JsValue::undefined());
    }

    if let Some(node) = ctx.element_tree.borrow().get(node_id) {
        if let Some(ref element) = node.element {
            if element.type_name() == "tur_pointer_interact" && key == "onClick" {
                let event_kind = ComposedGestureEventKind::Click;
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
                ctx.push_event(AppEvent::RequestDraw);
                return Ok(JsValue::undefined());
            }
        }
    }

    if let Some(node) = ctx.element_tree.borrow_mut().get_mut(node_id) {
        if let Some(ref mut element) = node.element {
            element.set_prop(context, &key, &value);
        }
    }

    ctx.push_event(AppEvent::RequestDraw);
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

    ctx.push_event(AppEvent::RequestDraw);
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

    ctx.push_event(AppEvent::RequestDraw);
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

    ctx.push_event(AppEvent::RequestDraw);
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
