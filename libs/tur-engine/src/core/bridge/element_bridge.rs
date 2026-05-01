use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::bridge::BoaOpaque;
use crate::core::bridge::TurJsContext;
use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementObject};
use crate::elements::{
    ContainerElement, FlexElement, FlexItemElement, FocusableElement, InputElement,
    PointerInteractElement, PositionedElement, StackElement, TextContainerElement, TextSpanElement,
};

#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurNodeHandle {
    pub(crate) id: ElementNodeId,
}

fn extract_ctx(args: &[JsValue]) -> JsResult<TurJsContext> {
    let obj = args.get_or_undefined(0).as_object().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("expected TurJsContext as first argument"),
        )
    })?;
    let ctx_ref = BoaOpaque::<TurJsContext>::wrap(&obj).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("expected TurJsContext as first argument"),
        )
    })?;
    Ok(ctx_ref.clone())
}

fn extract_node_id(args: &[JsValue], idx: usize) -> JsResult<ElementNodeId> {
    let obj = args.get_or_undefined(idx).as_object().ok_or_else(|| {
        JsNativeError::typ().with_message("expected TurNodeHandle")
    })?;
    let handle = BoaOpaque::<TurNodeHandle>::wrap(&obj).ok_or_else(|| {
        JsNativeError::typ().with_message("expected TurNodeHandle")
    })?;
    Ok(handle.id)
}

fn create_element(
    args: &[JsValue],
    context: &mut Context,
    element: AnyElement,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let mut tree = js_ctx.element_tree.borrow_mut();
    let id = tree.alloc_id();
    let node = ElementObject::new(id, element, context);
    tree.insert(node);
    let handle = tree.get(id).unwrap().handle.clone();
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
        AnyElement::new(PointerInteractElement::new())
            .with_js_command_emitter::<PointerInteractElement>(),
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
    create_element(
        args,
        context,
        AnyElement::with_focusability(FocusableElement::new())
            .with_js_command_emitter::<FocusableElement>(),
    )
}

pub(crate) fn tur_request_focus(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let node_id = extract_node_id(args, 1)?;
    let mut focus = js_ctx.focus_manager.borrow_mut();
    let mut js_eq = js_ctx.js_command_queue.borrow_mut();
    focus.set_focus(node_id, &mut js_eq);
    Ok(JsValue::undefined())
}

pub(crate) fn tur_set_attribute(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
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
                let mut tree = js_ctx.element_tree.borrow_mut();
                if let Some(node) = tree.get_mut(node_id) {
                    node.query_key = if keys.is_empty() { None } else { Some(keys) };
                }
            }
        }
        js_ctx.dirty.set(true);
        return Ok(JsValue::undefined());
    }

    {
        let mut tree = js_ctx.element_tree.borrow_mut();
        if let Some(node) = tree.get_mut(node_id) {
            if let Some(ref mut element) = node.element {
                element.set_prop(context, &key, &value);
            }
        }
    }

    js_ctx.dirty.set(true);
    Ok(JsValue::undefined())
}

pub(crate) fn tur_append_child(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let parent_id = extract_node_id(args, 1)?;
    let child_id = extract_node_id(args, 2)?;

    js_ctx.element_tree.borrow_mut().append_child(parent_id, child_id);

    tracing::trace!("tur_appendChild({}, {})", parent_id, child_id);

    js_ctx.dirty.set(true);
    Ok(JsValue::undefined())
}

pub(crate) fn tur_remove_child(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let parent_id = extract_node_id(args, 1)?;
    let child_id = extract_node_id(args, 2)?;

    js_ctx.element_tree.borrow_mut().remove_child(parent_id, child_id);

    tracing::trace!("tur_removeChild({}, {})", parent_id, child_id);

    js_ctx.dirty.set(true);
    Ok(JsValue::undefined())
}

pub(crate) fn tur_insert_before(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let parent_id = extract_node_id(args, 1)?;
    let child_id = extract_node_id(args, 2)?;
    let ref_id = extract_node_id(args, 3)?;

    js_ctx.element_tree
        .borrow_mut()
        .insert_before(parent_id, child_id, ref_id);

    tracing::trace!("tur_insertBefore({}, {}, {})", parent_id, child_id, ref_id);

    js_ctx.dirty.set(true);
    Ok(JsValue::undefined())
}

pub(crate) fn tur_get_parent(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let node_id = extract_node_id(args, 1)?;
    let tree = js_ctx.element_tree.borrow();
    match tree.parent_of(node_id) {
        Some(parent_id) => {
            let handle = tree.get(parent_id).unwrap().handle.clone();
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
    let js_ctx = extract_ctx(args)?;
    let node_id = extract_node_id(args, 1)?;
    let tree = js_ctx.element_tree.borrow();
    match tree.first_child_of(node_id) {
        Some(child_id) => {
            let handle = tree.get(child_id).unwrap().handle.clone();
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
    let js_ctx = extract_ctx(args)?;
    let node_id = extract_node_id(args, 1)?;
    let tree = js_ctx.element_tree.borrow();
    match tree.next_sibling_of(node_id) {
        Some(sibling_id) => {
            let handle = tree.get(sibling_id).unwrap().handle.clone();
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
    let js_ctx = extract_ctx(args)?;
    let mut tree = js_ctx.element_tree.borrow_mut();
    let id = tree.alloc_id();
    let element = AnyElement::with_full_interactivity(InputElement::new())
        .with_js_command_emitter::<InputElement>();
    let node = ElementObject::new(id, element, context);
    tree.insert(node);
    let handle = tree.get(id).unwrap().handle.clone();
    Ok(handle.object().clone().into())
}

pub(crate) fn tur_set_input_text(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let node_id = extract_node_id(args, 1)?;
    let text = args
        .get_or_undefined(2)
        .as_string()
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    {
        let mut tree = js_ctx.element_tree.borrow_mut();
        if let Some(node) = tree.get_mut(node_id) {
            if let Some(ref mut element) = node.element {
                if let Some(input_el) = element.cast_mut::<InputElement>() {
                    input_el.set_text(&text);
                }
            }
        }
    }
    js_ctx.dirty.set(true);
    Ok(JsValue::undefined())
}
