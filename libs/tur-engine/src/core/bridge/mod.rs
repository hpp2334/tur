use boa_engine::context::time::FixedClock;
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::property::PropertyDescriptor;
use boa_engine::Context;

use crate::core::app::TurAppInternal;
use crate::core::bridge::color::{tur_create_color, tur_create_linear_gradient};
use crate::core::bridge::reactive_bridge::{
    tur_component, tur_derive, tur_get, tur_mutate, tur_set, tur_source,
};
use crate::core::bridge::utils::{
    tur_create_animation_controller, tur_create_image_resource, tur_create_lazy_list_controller,
    tur_create_scroll_controller, tur_create_text_editing_controller, tur_request_focus,
};
use crate::core::bridge::widget_bridge::{
    tur_column, tur_condition, tur_container, tur_dynamic, tur_expanded, tur_focusable,
    tur_fragment, tur_image_edgy, tur_input_edgy, tur_lazy_list, tur_match, tur_pointer_interact,
    tur_positioned, tur_render, tur_row, tur_scroll_view, tur_stack, tur_svg_edgy, tur_text,
};
use crate::core::fonts::FontLoader;
use crate::core::render::Renderer;

pub(crate) mod color;
pub(crate) mod console;
pub(crate) mod executor;
pub(crate) mod reactive_bridge;
pub(crate) mod utils;
pub(crate) mod widget_bridge;
mod js_context;
mod opaque;
pub(crate) mod timer;

pub use executor::TurJobExecutor;
pub use console::register_console_globals;
pub use js_context::TurJsContext;
pub use opaque::BoaOpaque;
pub use timer::TimerState;
pub use utils::TurNodeHandle;

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

    context
        .register_global_class::<crate::elements::LazyListController>()
        .expect("failed to register LazyListController class");

    context
        .register_global_class::<crate::core::animation::AnimationController>()
        .expect("failed to register AnimationController class");

    let fns: [(
        &str,
        usize,
        boa_engine::native_function::NativeFunctionPointer,
    ); 28] = [
        // --- reactive primitives ---
        ("source", 2, tur_source),
        ("derive", 2, tur_derive),
        ("mutate", 2, tur_mutate),
        ("get", 2, tur_get),
        ("set", 3, tur_set),
        ("component", 1, tur_component),
        // --- widget spec factories ---
        ("Container", 2, tur_container),
        ("Column", 2, tur_column),
        ("Row", 2, tur_row),
        ("Expanded", 2, tur_expanded),
        ("Stack", 2, tur_stack),
        ("Positioned", 2, tur_positioned),
        ("Text", 2, tur_text),
        ("PointerInteract", 2, tur_pointer_interact),
        ("Condition", 2, tur_condition),
        ("Match", 2, tur_match),
        ("Dynamic", 2, tur_dynamic),
        ("LazyList", 2, tur_lazy_list),
        ("ScrollView", 2, tur_scroll_view),
        ("ImageEdgy", 2, tur_image_edgy),
        ("InputEdgy", 2, tur_input_edgy),
        ("Fragment", 2, tur_fragment),
        ("SvgEdgy", 2, tur_svg_edgy),
        ("Focusable", 2, tur_focusable),
        ("render", 2, tur_render),
        // --- utility functions ---
        ("createTextEditingController", 2, tur_create_text_editing_controller),
        ("createScrollController", 2, tur_create_scroll_controller),
        ("createLazyListController", 2, tur_create_lazy_list_controller),
    ];

    for (name, length, ptr) in &fns {
        let js_name = js_string!(*name);
        let func = build_fn(context, &js_name, *length, *ptr);
        set_prop(&tur_obj, js_name.clone(), func);
    }

    // Register remaining utility functions separately (variadic lengths).
    let js_name = js_string!("createColor");
    set_prop(&tur_obj, js_name.clone(), build_fn(context, &js_name, 4, tur_create_color));
    let js_name = js_string!("createLinearGradient");
    set_prop(&tur_obj, js_name.clone(), build_fn(context, &js_name, 5, tur_create_linear_gradient));
    let js_name = js_string!("createAnimationController");
    set_prop(
        &tur_obj,
        js_name.clone(),
        build_fn(context, &js_name, 2, tur_create_animation_controller),
    );
    let js_name = js_string!("createImageResource");
    set_prop(
        &tur_obj,
        js_name.clone(),
        build_fn(context, &js_name, 2, tur_create_image_resource),
    );
    let js_name = js_string!("requestFocus");
    set_prop(
        &tur_obj,
        js_name.clone(),
        build_fn(context, &js_name, 2, tur_request_focus),
    );

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
