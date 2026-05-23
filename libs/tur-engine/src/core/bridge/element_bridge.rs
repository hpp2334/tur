use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::property::PropertyDescriptor;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::bridge::BoaOpaque;
use crate::core::bridge::TurJsContext;
use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementObject};
use crate::core::resource::ImageResource;
use crate::elements::{
    ContainerElement, EditableTextElement, FlexElement, FlexItemElement,
    FocusableElement, ImageElement, PointerInteractElement, PositionedElement,
    StackElement, TextContainerElement,
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
    let _kind = element.type_name().to_string();
    let node = ElementObject::new(id, element, context);
    tree.insert(node);
    let handle = tree.get(id).unwrap().handle.clone();
    Ok(handle.object().clone().into())
}

macro_rules! simple_creator {
    ($fn_name:ident, $constructor:expr) => {
        pub(crate) fn $fn_name(
            _this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            create_element(args, context, $constructor)
        }
    };
}

simple_creator!(tur_create_flex, AnyElement::new(FlexElement::new()));
simple_creator!(tur_create_flex_item, AnyElement::new(FlexItemElement::new()));
simple_creator!(tur_create_stack, AnyElement::new(StackElement::new()));
simple_creator!(tur_create_positioned, AnyElement::new(PositionedElement::new()));
simple_creator!(tur_create_container, AnyElement::new(ContainerElement::new()));
simple_creator!(tur_create_text_container, AnyElement::new(TextContainerElement::new()));
simple_creator!(tur_create_image, AnyElement::new(ImageElement::new()));

pub(crate) fn tur_create_pointer_interact(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    create_element(
        args,
        context,
        AnyElement::new(PointerInteractElement::new())
            .with_js_callback_emitter::<PointerInteractElement>(),
    )
}

pub(crate) fn tur_create_root(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    create_element(args, context, AnyElement::new(FlexElement::new()))
}

pub(crate) fn tur_create_focusable(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    create_element(
        args,
        context,
        AnyElement::with_focusability(FocusableElement::new())
            .with_js_callback_emitter::<FocusableElement>(),
    )
}

pub(crate) fn tur_create_editable_text(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    create_element(
        args,
        context,
        AnyElement::with_full_interactivity(EditableTextElement::new())
            .with_js_callback_emitter::<EditableTextElement>(),
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
        js_ctx.element_tree.borrow_mut().mark_dirty(node_id);
        js_ctx.dirty.set(true);
        return Ok(JsValue::undefined());
    }

    {
        let mut tree = js_ctx.element_tree.borrow_mut();
        if let Some(node) = tree.get_mut(node_id) {
            if let Some(ref mut element) = node.element {
                if value.is_null() || value.is_undefined() {
                    element.reset_prop(&key);
                } else {
                    element.set_prop(context, &key, &value);
                }
            }
        }
        tree.mark_dirty(node_id);
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

    let mut tree = js_ctx.element_tree.borrow_mut();
    tree.append_child(parent_id, child_id);
    tree.mark_dirty(parent_id);

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

    let mut tree = js_ctx.element_tree.borrow_mut();
    tree.remove_child(parent_id, child_id);
    tree.mark_dirty(parent_id);

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

    let mut tree = js_ctx.element_tree.borrow_mut();
    tree.insert_before(parent_id, child_id, ref_id);
    tree.mark_dirty(parent_id);

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

pub(crate) fn tur_get_text_cursor_rect(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let node_id = extract_node_id(args, 1)?;
    let char_index = args.get_or_undefined(2)
        .as_number()
        .map(|n| n as usize)
        .unwrap_or(0);

    let tree = js_ctx.element_tree.borrow();
    let Some(node) = tree.get(node_id) else {
        return Ok(JsValue::null());
    };
    let Some(ref element) = node.element else {
        return Ok(JsValue::null());
    };

    let layout = element.cast::<TextContainerElement>()
        .and_then(|e| e.cached_layout.as_ref())
        .or_else(|| {
            element.cast::<EditableTextElement>()
                .and_then(|e| e.cached_layout.as_ref())
        });

    let Some(layout_data) = layout else {
        return Ok(JsValue::null());
    };

    let (x, _) = layout_data.cursor_xy_at(char_index);
    let line_idx = layout_data.line_index_for_char(char_index);
    let line_info = &layout_data.line_infos[line_idx];

    let proto = context.intrinsics().constructors().object().prototype();
    let obj = JsObject::from_proto_and_data(proto, ());
    let desc = |v: f64| {
        PropertyDescriptor::builder()
            .value(JsValue::from(v))
            .writable(true)
            .enumerable(true)
            .configurable(true)
            .build()
    };
    obj.insert_property(js_string!("x"), desc(x as f64));
    obj.insert_property(js_string!("y"), desc(line_info.top as f64));
    obj.insert_property(js_string!("w"), desc(2.0_f64));
    obj.insert_property(js_string!("h"), desc(line_info.height as f64));

    Ok(obj.into())
}

pub(crate) fn tur_get_text_selection_rects(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let node_id = extract_node_id(args, 1)?;
    let start_char = args.get_or_undefined(2)
        .as_number()
        .map(|n| n as usize)
        .unwrap_or(0);
    let end_char = args.get_or_undefined(3)
        .as_number()
        .map(|n| n as usize)
        .unwrap_or(0);

    let tree = js_ctx.element_tree.borrow();
    let Some(node) = tree.get(node_id) else {
        return Ok(JsValue::from(boa_engine::object::builtins::JsArray::new(context).unwrap()));
    };
    let Some(ref element) = node.element else {
        return Ok(JsValue::from(boa_engine::object::builtins::JsArray::new(context).unwrap()));
    };

    let layout = element.cast::<TextContainerElement>()
        .and_then(|e| e.cached_layout.as_ref())
        .or_else(|| {
            element.cast::<EditableTextElement>()
                .and_then(|e| e.cached_layout.as_ref())
        });

    let Some(layout_data) = layout else {
        return Ok(JsValue::from(boa_engine::object::builtins::JsArray::new(context).unwrap()));
    };

    let (s, e) = if start_char < end_char { (start_char, end_char) } else { (end_char, start_char) };
    let start_line = layout_data.line_index_for_char(s);
    let end_line = layout_data.line_index_for_char(e);

    let mut rects = Vec::new();
    for line_idx in start_line..=end_line {
        let line_start = layout_data.line_start_char(line_idx);
        let line_end = layout_data.line_end_char(line_idx);

        let sel_start = s.max(line_start);
        let sel_end = e.min(line_end);

        if sel_start >= sel_end {
            continue;
        }

        let x_start = layout_data.cursor_x_at(sel_start);
        let x_end = layout_data.cursor_x_at(sel_end);
        let line_info = &layout_data.line_infos[line_idx];

        let proto = context.intrinsics().constructors().object().prototype();
        let obj = JsObject::from_proto_and_data(proto, ());
        let desc = |v: f64| {
            PropertyDescriptor::builder()
                .value(JsValue::from(v))
                .writable(true)
                .enumerable(true)
                .configurable(true)
                .build()
        };
        obj.insert_property(js_string!("x"), desc(x_start as f64));
        obj.insert_property(js_string!("y"), desc(line_info.top as f64));
        obj.insert_property(js_string!("w"), desc((x_end - x_start) as f64));
        obj.insert_property(js_string!("h"), desc(line_info.height as f64));
        rects.push(JsValue::from(obj));
    }

    let arr = boa_engine::object::builtins::JsArray::from_iter(rects, context);
    Ok(JsValue::from(arr))
}

pub(crate) fn tur_get_char_index_at_position(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let node_id = extract_node_id(args, 1)?;
    let x = args.get_or_undefined(2).as_number().unwrap_or(0.0) as f32;
    let y = args.get_or_undefined(3).as_number().unwrap_or(0.0) as f32;

    let tree = js_ctx.element_tree.borrow();
    let Some(node) = tree.get(node_id) else {
        return Ok(JsValue::from(0.0_f64));
    };
    let Some(ref element) = node.element else {
        return Ok(JsValue::from(0.0_f64));
    };

    let layout = element.cast::<TextContainerElement>()
        .and_then(|e| e.cached_layout.as_ref())
        .or_else(|| {
            element.cast::<EditableTextElement>()
                .and_then(|e| e.cached_layout.as_ref())
        });

    let Some(layout_data) = layout else {
        return Ok(JsValue::from(0.0_f64));
    };

    let char_index = layout_data.char_index_at_xy(x, y);
    Ok(JsValue::from(char_index as f64))
}

pub(crate) fn tur_create_image_resource(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;

    let buffer_obj = args.get_or_undefined(1).as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected ArrayBuffer or Uint8Array"))
    })?;

    let bytes = if let Ok(ta) = boa_engine::object::builtins::JsTypedArray::from_object(buffer_obj.clone()) {
        let offset = ta.byte_offset(context).unwrap_or(0);
        let len = ta.byte_length(context).unwrap_or(0);
        let buffer_val = ta.buffer(context)?;
        let buffer_obj = buffer_val.as_object().ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message("typed array has no backing buffer"))
        })?;
        let ab = boa_engine::object::builtins::JsArrayBuffer::from_object(buffer_obj)?;
        let full = ab.to_vec().unwrap_or_default();
        if offset + len > full.len() {
            full
        } else {
            full[offset..offset + len].to_vec()
        }
    } else if let Ok(ab) = boa_engine::object::builtins::JsArrayBuffer::from_object(buffer_obj.clone()) {
        ab.to_vec().unwrap_or_default()
    } else {
        return Err(JsError::from(
            JsNativeError::typ().with_message("expected ArrayBuffer or Uint8Array"),
        ));
    };

    let image_resource = ImageResource::from_bytes(&bytes).ok_or_else(|| {
        JsError::from(
            JsNativeError::range().with_message("failed to decode image (supported: PNG, JPEG)"),
        )
    })?;

    let resource_id = {
        let mut resource_map = js_ctx.resource_map.borrow_mut();
        resource_map.insert_image(image_resource)
    };

    Ok(JsValue::from(resource_id.as_u64() as f64))
}
