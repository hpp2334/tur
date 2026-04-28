use std::cell::RefCell;
use std::rc::{Rc, Weak};

use boa_engine::object::JsObject;
use boa_engine::{Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::app::TurAppInternal;
use crate::core::bridge::BoaOpaque;
use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementObject};
use crate::core::event::AppEvent;
use crate::core::focus::FocusEventType;
use crate::core::gesture::ComposedGestureEventKind;
use crate::core::keyboard::KeyEventType;
use crate::elements::{
    ContainerElement, FlexElement, FlexItemElement, FocusableElement, InputElement,
    PointerInteractElement, PositionedElement, StackElement, TextContainerElement, TextSpanElement,
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
    let mut ctx = ctx.borrow_mut();
    let id = ctx.element_tree_mut().alloc_id();
    let node = ElementObject::new(id, element, context);
    ctx.element_tree_mut().insert(node);
    let handle = ctx.element_tree().get(id).unwrap().handle.clone();
    Ok(handle.object().clone().into())
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

pub(crate) fn tur_create_text_container(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createTextContainer()");
    create_element(
        args,
        context,
        AnyElement::new(TextContainerElement::new()),
    )
}

pub(crate) fn tur_create_text_span(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createTextSpan()");
    create_element(args, context, AnyElement::new(TextSpanElement::new()))
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

pub(crate) fn tur_create_focusable(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createFocusable()");
    create_element(args, context, AnyElement::new(FocusableElement::new()))
}

pub(crate) fn tur_request_focus(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = extract_ctx(args)?;
    let node_id = extract_node_id(args, 1)?;
    let (blur_cb, focus_cb) = {
        let mut ctx = ctx.borrow_mut();
        let old_id = ctx.request_focus(node_id);
        let blur_cb = if let Some(old) = old_id {
            ctx.collect_focus_handler(old, FocusEventType::Blur)
        } else {
            None
        };
        let focus_cb = ctx.collect_focus_handler(node_id, FocusEventType::Focus);
        (blur_cb, focus_cb)
    };
    if let Some(callback) = blur_cb {
        let _ = callback.call(&JsValue::undefined(), &[], context);
    }
    if let Some(callback) = focus_cb {
        let _ = callback.call(&JsValue::undefined(), &[], context);
    }
    Ok(JsValue::undefined())
}

pub(crate) fn tur_set_attribute(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = extract_ctx(args)?;
    let mut ctx = ctx.borrow_mut();
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
                ctx.set_node_query_key(node_id, if keys.is_empty() { None } else { Some(keys) });
            }
        }
        ctx.push_event(AppEvent::RequestDraw);
        return Ok(JsValue::undefined());
    }

    let element_type = ctx
        .element_tree()
        .get(node_id)
        .and_then(|n| n.element.as_ref())
        .map(|e| e.type_name().to_string());

    let Some(element_type) = element_type else {
        if let Some(node) = ctx.element_tree_mut().get_mut(node_id) {
            if let Some(ref mut element) = node.element {
                element.set_prop(context, &key, &value);
            }
        }
        ctx.push_event(AppEvent::RequestDraw);
        return Ok(JsValue::undefined());
    };

    if element_type == "tur_pointer_interact" && key == "onClick" {
        let event_kind = ComposedGestureEventKind::Click;
        if let Some(obj) = value.as_object() {
            if obj.is_callable() {
                ctx.set_event_handler(node_id, event_kind, obj.clone());
            }
        } else if value.is_null() || value.is_undefined() {
            ctx.remove_event_handler(node_id, event_kind);
        }
        ctx.push_event(AppEvent::RequestDraw);
        return Ok(JsValue::undefined());
    }

    if element_type == "tur_focusable" {
        let handled = match key.to_std_string_escaped().as_str() {
            "onKeyDown" | "onKeyUp" => {
                let key_event_type = if key == "onKeyDown" {
                    KeyEventType::Down
                } else {
                    KeyEventType::Up
                };
                if let Some(obj) = value.as_object() {
                    if obj.is_callable() {
                        ctx.set_key_handler(node_id, key_event_type, obj.clone());
                    }
                } else if value.is_null() || value.is_undefined() {
                    ctx.remove_key_handler(node_id, key_event_type);
                }
                true
            }
            "onFocus" | "onBlur" => {
                let focus_event_type = if key == "onFocus" {
                    FocusEventType::Focus
                } else {
                    FocusEventType::Blur
                };
                if let Some(obj) = value.as_object() {
                    if obj.is_callable() {
                        ctx.set_focus_handler(node_id, focus_event_type, obj.clone());
                    }
                } else if value.is_null() || value.is_undefined() {
                    ctx.remove_focus_handler(node_id, focus_event_type);
                }
                true
            }
            _ => false,
        };
        if handled {
            ctx.push_event(AppEvent::RequestDraw);
            return Ok(JsValue::undefined());
        }
    }

    if element_type == "tur_input" {
        let handled = match key.to_std_string_escaped().as_str() {
            "onInput" => {
                if let Some(obj) = value.as_object() {
                    if obj.is_callable() {
                        ctx.text_input_callbacks.insert(node_id, obj.clone());
                    }
                } else if value.is_null() || value.is_undefined() {
                    ctx.text_input_callbacks.remove(&node_id);
                }
                true
            }
            "onFocus" => {
                if let Some(obj) = value.as_object() {
                    if obj.is_callable() {
                        ctx.text_input_focus_handlers
                            .insert((node_id, FocusEventType::Focus), obj.clone());
                    }
                } else if value.is_null() || value.is_undefined() {
                    ctx.text_input_focus_handlers
                        .remove(&(node_id, FocusEventType::Focus));
                }
                true
            }
            "onBlur" => {
                if let Some(obj) = value.as_object() {
                    if obj.is_callable() {
                        ctx.text_input_focus_handlers
                            .insert((node_id, FocusEventType::Blur), obj.clone());
                    }
                } else if value.is_null() || value.is_undefined() {
                    ctx.text_input_focus_handlers
                        .remove(&(node_id, FocusEventType::Blur));
                }
                true
            }
            "onCursorChange" => {
                if let Some(obj) = value.as_object() {
                    if obj.is_callable() {
                        ctx.text_input_cursor_handlers.insert(node_id, obj.clone());
                    }
                } else if value.is_null() || value.is_undefined() {
                    ctx.text_input_cursor_handlers.remove(&node_id);
                }
                true
            }
            "onSelectionChange" => {
                if let Some(obj) = value.as_object() {
                    if obj.is_callable() {
                        ctx.text_input_selection_handlers.insert(node_id, obj.clone());
                    }
                } else if value.is_null() || value.is_undefined() {
                    ctx.text_input_selection_handlers.remove(&node_id);
                }
                true
            }
            _ => false,
        };
        if handled {
            ctx.push_event(AppEvent::RequestDraw);
            return Ok(JsValue::undefined());
        }
    }

    if let Some(node) = ctx.element_tree_mut().get_mut(node_id) {
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
    let mut ctx = ctx.borrow_mut();
    let parent_id = extract_node_id(args, 1)?;
    let child_id = extract_node_id(args, 2)?;

    ctx.element_tree_mut().append_child(parent_id, child_id);

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
    let mut ctx = ctx.borrow_mut();
    let parent_id = extract_node_id(args, 1)?;
    let child_id = extract_node_id(args, 2)?;

    ctx.element_tree_mut().remove_child(parent_id, child_id);

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
    let mut ctx = ctx.borrow_mut();
    let parent_id = extract_node_id(args, 1)?;
    let child_id = extract_node_id(args, 2)?;
    let ref_id = extract_node_id(args, 3)?;

    ctx.element_tree_mut()
        .insert_before(parent_id, child_id, ref_id);

    tracing::trace!("tur_insertBefore({}, {}, {})", parent_id, child_id, ref_id);

    ctx.push_event(AppEvent::RequestDraw);
    Ok(JsValue::undefined())
}

pub(crate) fn tur_get_parent(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow_mut();
    let node_id = extract_node_id(args, 1)?;
    match ctx.element_tree().parent_of(node_id) {
        Some(parent_id) => {
            let handle = ctx.element_tree().get(parent_id).unwrap().handle.clone();
            Ok(handle.object().clone().into())
        }
        None => Ok(JsValue::null()),
    }
}

pub(crate) fn tur_get_first_child(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow_mut();
    let node_id = extract_node_id(args, 1)?;
    match ctx.element_tree().first_child_of(node_id) {
        Some(child_id) => {
            let handle = ctx.element_tree().get(child_id).unwrap().handle.clone();
            Ok(handle.object().clone().into())
        }
        None => Ok(JsValue::null()),
    }
}

pub(crate) fn tur_get_next_sibling(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = extract_ctx(args)?;
    let ctx = ctx.borrow_mut();
    let node_id = extract_node_id(args, 1)?;
    match ctx.element_tree().next_sibling_of(node_id) {
        Some(sibling_id) => {
            let handle = ctx.element_tree().get(sibling_id).unwrap().handle.clone();
            Ok(handle.object().clone().into())
        }
        None => Ok(JsValue::null()),
    }
}

pub(crate) fn tur_create_input(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    tracing::trace!("tur_createInput()");
    let ctx = extract_ctx(args)?;
    let mut ctx = ctx.borrow_mut();
    let id = ctx.element_tree_mut().alloc_id();
    let element = AnyElement::new(InputElement::new());
    let node = ElementObject::new(id, element, context);
    ctx.element_tree_mut().insert(node);
    ctx.input_nodes.insert(id);
    let handle = ctx.element_tree().get(id).unwrap().handle.clone();
    Ok(handle.object().clone().into())
}

pub(crate) fn tur_set_input_text(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = extract_ctx(args)?;
    let mut ctx = ctx.borrow_mut();
    let node_id = extract_node_id(args, 1)?;
    let text = args
        .get_or_undefined(2)
        .as_string()
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    if let Some(node) = ctx.element_tree_mut().get_mut(node_id) {
        if let Some(ref mut element) = node.element {
            if let Some(input_el) = element.cast_mut::<InputElement>() {
                input_el.set_text(&text);
            }
        }
    }
    ctx.push_event(AppEvent::RequestDraw);
    Ok(JsValue::undefined())
}

pub(crate) fn build_key_event_object(
    key: &str,
    code: &str,
    modifiers: &crate::core::keyboard::Modifiers,
    context: &mut Context,
) -> JsValue {
    let proto = context.intrinsics().constructors().object().prototype();
    let obj = JsObject::from_proto_and_data(proto, ());

    let desc = boa_engine::property::PropertyDescriptor::builder()
        .writable(true)
        .enumerable(true)
        .configurable(true);

    obj.insert_property(
        boa_engine::js_string!("key"),
        desc.clone()
            .value(boa_engine::JsValue::from(boa_engine::js_string!(key)))
            .build(),
    );
    obj.insert_property(
        boa_engine::js_string!("code"),
        desc.clone()
            .value(boa_engine::JsValue::from(boa_engine::js_string!(code)))
            .build(),
    );
    obj.insert_property(
        boa_engine::js_string!("ctrl"),
        desc.clone()
            .value(boa_engine::JsValue::from(modifiers.ctrl))
            .build(),
    );
    obj.insert_property(
        boa_engine::js_string!("shift"),
        desc.clone()
            .value(boa_engine::JsValue::from(modifiers.shift))
            .build(),
    );
    obj.insert_property(
        boa_engine::js_string!("alt"),
        desc.clone()
            .value(boa_engine::JsValue::from(modifiers.alt))
            .build(),
    );
    obj.insert_property(
        boa_engine::js_string!("meta"),
        desc.value(boa_engine::JsValue::from(modifiers.meta))
            .build(),
    );

    obj.into()
}
