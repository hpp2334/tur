pub(crate) mod color;
pub(crate) mod executor;
pub(crate) mod console;
pub(crate) mod element_bridge;
mod js_context;
mod opaque;
pub(crate) mod timer;

pub use executor::TurJobExecutor;
pub use console::register_console_globals;
pub use element_bridge::TurNodeHandle;
pub use js_context::TurJsContext;
pub use opaque::BoaOpaque;
pub use timer::TimerState;

use boa_engine::context::time::FixedClock;
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::property::PropertyDescriptor;
use boa_engine::Context;

use crate::core::app::TurAppInternal;
use crate::core::bridge::element_bridge::{
    tur_append_child, tur_create_container, tur_create_editable_text,
    tur_create_flex, tur_create_flex_item, tur_create_focusable, tur_create_image,
    tur_create_image_resource, tur_create_pointer_interact, tur_create_positioned,
    tur_create_root, tur_create_scroll_controller, tur_create_scroll_view, tur_create_stack,
    tur_create_paragraph, tur_create_text_editing_controller,
    tur_get_char_index_at_position, tur_get_first_child, tur_get_next_sibling, tur_get_parent,
    tur_get_text_cursor_rect, tur_get_text_selection_rects, tur_insert_before, tur_remove_child,
    tur_request_focus, tur_set_attribute,
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

pub struct BridgeResult {
    pub internal: TurAppInternal,
    pub clock: std::rc::Rc<FixedClock>,
    pub executor: std::rc::Rc<TurJobExecutor>,
}

pub fn init_bridge(
    context: &mut Context,
    renderer: Box<dyn Renderer>,
    font_loader: Box<dyn FontLoader>,
    clock: std::rc::Rc<FixedClock>,
    executor: std::rc::Rc<TurJobExecutor>,
) -> BridgeResult {
    let proto = context.intrinsics().constructors().object().prototype();
    let tur_obj = JsObject::from_proto_and_data(proto, ());

    context
        .register_global_class::<crate::core::text::TextEditingController>()
        .expect("failed to register TextEditingController class");

    context
        .register_global_class::<crate::core::scroll::ScrollController>()
        .expect("failed to register ScrollController class");

    let fns: [(
        &str,
        usize,
        boa_engine::native_function::NativeFunctionPointer,
    ); 26] = [
        ("createFlex", 1, tur_create_flex),
        ("createFlexItem", 1, tur_create_flex_item),
        ("createStack", 1, tur_create_stack),
        ("createPositioned", 1, tur_create_positioned),
        ("createContainer", 1, tur_create_container),
        ("createParagraph", 1, tur_create_paragraph),
        ("createPointerInteract", 1, tur_create_pointer_interact),
        ("createFocusable", 1, tur_create_focusable),
        ("createEditableText", 2, tur_create_editable_text),
        ("createImage", 1, tur_create_image),
        ("createScrollView", 1, tur_create_scroll_view),
        ("createImageResource", 2, tur_create_image_resource),
        ("createRoot", 1, tur_create_root),
        ("createTextEditingController", 2, tur_create_text_editing_controller),
        ("createScrollController", 2, tur_create_scroll_controller),
        ("setAttribute", 4, tur_set_attribute),
        ("appendChild", 3, tur_append_child),
        ("removeChild", 3, tur_remove_child),
        ("insertBefore", 4, tur_insert_before),
        ("getParent", 2, tur_get_parent),
        ("getFirstChild", 2, tur_get_first_child),
        ("getNextSibling", 2, tur_get_next_sibling),
        ("requestFocus", 2, tur_request_focus),
        ("getTextCursorRect", 3, tur_get_text_cursor_rect),
        ("getTextSelectionRects", 4, tur_get_text_selection_rects),
        ("getCharIndexAtPosition", 4, tur_get_char_index_at_position),
    ];

    for (name, length, ptr) in &fns {
        let js_name = js_string!(*name);
        let func = build_fn(context, &js_name, *length, *ptr);
        set_prop(&tur_obj, js_name.clone(), func);
    }

    let internal = TurAppInternal::new(renderer, font_loader, executor.clone());
    {
        let mut ctx = internal.app_context.borrow_mut();
        ctx.register_handler(Box::new(
            crate::handlers::text_edit_focus::TextEditFocusAppHandler,
        ));
        ctx.register_handler(Box::new(
            crate::handlers::gesture::GestureAppHandler,
        ));
        ctx.register_handler(Box::new(
            crate::handlers::keyboard::KeyboardAppHandler,
        ));
        ctx.register_handler(Box::new(
            crate::handlers::ime::ImeAppHandler,
        ));
        ctx.register_handler(Box::new(
            crate::handlers::resize::ResizeHandler,
        ));
        ctx.register_handler(Box::new(
            crate::handlers::pointer_region::PointerRegionAppHandler::new(),
        ));
        ctx.register_handler(Box::new(
            crate::handlers::wheel::WheelAppHandler,
        ));
        ctx.register_handler(Box::new(
            crate::handlers::scroll_chaining::ScrollChainingHandler,
        ));
    }

    let opaque = BoaOpaque::new(internal.js_context.clone(), context);

    set_prop(
        &tur_obj,
        js_string!("__ctx"),
        Into::<boa_engine::JsValue>::into(opaque.object().clone()),
    );

    context
        .register_global_property(js_string!("__tur"), tur_obj, Attribute::all())
        .expect("failed to register __tur global");

    let schedule_flush = internal.needs_draw.clone();
    let timer_state = std::rc::Rc::new(std::cell::RefCell::new(TimerState::new()));
    timer::register_timer_globals(context, timer_state, schedule_flush);
    console::register_console_globals(context);

    tracing::info!("tur bridge initialized");

    BridgeResult { internal, clock, executor }
}
