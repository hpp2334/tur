pub(crate) mod element_bridge;
mod opaque;

pub use element_bridge::{TurNodeHandle, WeakAppContext};
pub use opaque::BoaOpaque;

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::property::PropertyDescriptor;
use boa_engine::Context;

use crate::core::app::TurAppInternal;
use crate::core::bridge::element_bridge::{
    tur_append_child, tur_create_container, tur_create_flex, tur_create_flex_item,
    tur_create_focusable, tur_create_pointer_interact, tur_create_positioned, tur_create_root,
    tur_create_stack, tur_create_text, tur_get_first_child, tur_get_next_sibling, tur_get_parent,
    tur_insert_before, tur_remove_child, tur_request_focus, tur_set_attribute,
};
use crate::core::fonts::FontLoader;
use crate::core::render::Renderer;

fn build_fn(
    context: &mut Context,
    name: &boa_engine::JsString,
    length: usize,
    f: boa_engine::native_function::NativeFunctionPointer,
) -> JsObject {
    boa_engine::object::FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(f))
        .name(name.clone())
        .length(length)
        .build()
        .into()
}

fn set_prop<K, V>(obj: &JsObject, key: K, value: V)
where
    K: Into<boa_engine::property::PropertyKey>,
    V: Into<boa_engine::JsValue>,
{
    let desc = PropertyDescriptor::builder()
        .value(value)
        .writable(true)
        .enumerable(false)
        .configurable(true)
        .build();
    obj.insert_property(key, desc);
}

pub fn init_bridge(
    context: &mut Context,
    renderer: Box<dyn Renderer>,
    font_loader: Box<dyn FontLoader>,
) -> Rc<RefCell<TurAppInternal>> {
    let proto = context.intrinsics().constructors().object().prototype();
    let tur_obj = JsObject::from_proto_and_data(proto, ());

    let fns: [(
        &str,
        usize,
        boa_engine::native_function::NativeFunctionPointer,
    ); 17] = [
        ("createFlex", 1, tur_create_flex),
        ("createFlexItem", 1, tur_create_flex_item),
        ("createStack", 1, tur_create_stack),
        ("createPositioned", 1, tur_create_positioned),
        ("createContainer", 1, tur_create_container),
        ("createText", 1, tur_create_text),
        ("createPointerInteract", 1, tur_create_pointer_interact),
        ("createFocusable", 1, tur_create_focusable),
        ("createRoot", 1, tur_create_root),
        ("setAttribute", 4, tur_set_attribute),
        ("appendChild", 3, tur_append_child),
        ("removeChild", 3, tur_remove_child),
        ("insertBefore", 4, tur_insert_before),
        ("getParent", 2, tur_get_parent),
        ("getFirstChild", 2, tur_get_first_child),
        ("getNextSibling", 2, tur_get_next_sibling),
        ("requestFocus", 2, tur_request_focus),
    ];

    for (name, length, ptr) in &fns {
        let js_name = js_string!(*name);
        let func = build_fn(context, &js_name, *length, *ptr);
        set_prop(&tur_obj, js_name.clone(), func);
    }

    let ctx = TurAppInternal::new(renderer, font_loader);
    let rc_ctx = Rc::new(RefCell::new(ctx));
    let weak = WeakAppContext::new(&rc_ctx);
    let opaque = BoaOpaque::new(weak, context);

    set_prop(
        &tur_obj,
        js_string!("__ctx"),
        Into::<boa_engine::JsValue>::into(opaque.object().clone()),
    );

    context
        .register_global_property(js_string!("__tur"), tur_obj, Attribute::all())
        .expect("failed to register __tur global");

    tracing::info!("tur bridge initialized");

    rc_ctx
}
