use boa_engine::context::time::FixedClock;
use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::Context;

use crate::core::app::TurAppInternal;
use crate::core::bridge::helpers::{ConstEntry, FnEntry};
use crate::core::fonts::FontLoader;
use crate::core::render::Renderer;

pub(crate) mod animation;
pub(crate) mod color;
pub(crate) mod console;
pub(crate) mod dev_tool;
pub(crate) mod enums;
pub(crate) mod executor;
pub(crate) mod helpers;
pub(crate) mod module_loader;
pub(crate) mod reactive;
pub(crate) mod render;
mod js_context;
mod opaque;
pub(crate) mod timer;

pub use console::register_console_globals;
pub use executor::TurJobExecutor;
pub use helpers::TurNodeHandle;
pub use js_context::TurJsContext;
pub use module_loader::TurModuleLoader;
pub use opaque::BoaOpaque;
pub use timer::TimerState;

pub struct BridgeResult {
    pub internal: TurAppInternal,
    pub executor: std::rc::Rc<TurJobExecutor>,
}

pub fn init_bridge(
    context: &mut Context,
    renderer: Box<dyn Renderer>,
    font_loader: Box<dyn FontLoader>,
    clock: std::rc::Rc<FixedClock>,
    platform_api: Box<dyn crate::core::platform_api::PlatformApi>,
    executor: std::rc::Rc<TurJobExecutor>,
    loader: std::rc::Rc<TurModuleLoader>,
) -> BridgeResult {
    context
        .register_global_class::<crate::core::text::TextEditingController>()
        .expect("failed to register TextEditingController class");
    context
        .register_global_class::<crate::core::text::UndoController>()
        .expect("failed to register UndoController class");
    context
        .register_global_class::<crate::core::scroll::ScrollController>()
        .expect("failed to register ScrollController class");
    context
        .register_global_class::<crate::elements::LazyListController>()
        .expect("failed to register LazyListController class");
    context
        .register_global_class::<crate::core::animation::AnimationController>()
        .expect("failed to register AnimationController class");

    let internal = TurAppInternal::new(renderer, font_loader, executor.clone(), clock, platform_api);
    {
        let mut ctx = internal.app_context.borrow_mut();
        ctx.register_handler(Box::new(crate::handlers::text_edit_focus::TextEditFocusAppHandler));
        ctx.register_handler(Box::new(crate::handlers::gesture::GestureAppHandler));
        ctx.register_handler(Box::new(crate::handlers::keyboard::KeyboardAppHandler));
        ctx.register_handler(Box::new(crate::handlers::ime::ImeAppHandler));
        ctx.register_handler(Box::new(crate::handlers::resize::ResizeHandler));
        ctx.register_handler(Box::new(
            crate::handlers::pointer_region::PointerRegionAppHandler::new(),
        ));
        ctx.register_handler(Box::new(crate::handlers::wheel::WheelAppHandler));
        ctx.register_handler(Box::new(crate::handlers::scroll_chaining::ScrollChainingHandler));
        ctx.register_handler(Box::new(crate::handlers::scroll_to::ScrollToHandler));
        ctx.register_handler(Box::new(crate::handlers::clipboard::ClipboardPasteHandler));
        ctx.register_handler(Box::new(crate::handlers::clipboard::ClipboardWriteHandler));
    }

    let opaque = BoaOpaque::new(internal.js_context.clone(), context);
    let ctx_val: boa_engine::JsValue = opaque.object().clone().into();

    // `builtin:tur/core` — the reactive core: atom primitives + event framework
    // (`mutate`/`set`) + `render`. No value types, no views, no consts.
    let mut core_fns: Vec<FnEntry> = Vec::new();
    core_fns.extend(reactive::fns());
    core_fns.extend(render::fns());
    let core_module = module_loader::build_native_module(
        context,
        opaque.object().clone().into(),
        &core_fns,
        &[],
    );
    loader.register("builtin:tur/core", core_module);

    // `builtin:tur/std` — the widget library + value types + event details.
    // Re-exports everything in core, then adds color, animation, all element
    // view factories/controllers/resources, and the enum + color const-objects.
    let mut std_fns = core_fns.clone();
    std_fns.extend(color::fns());
    std_fns.extend(animation::fns());
    std_fns.extend(crate::elements::container::bridge::fns());
    std_fns.extend(crate::elements::flex::bridge::fns());
    std_fns.extend(crate::elements::flex_item::bridge::fns());
    std_fns.extend(crate::elements::stack::bridge::fns());
    std_fns.extend(crate::elements::positioned::bridge::fns());
    std_fns.extend(crate::elements::paragraph::bridge::fns());
    std_fns.extend(crate::elements::editable_text::bridge::fns());
    std_fns.extend(crate::elements::image::bridge::fns());
    std_fns.extend(crate::elements::pointer_interact::bridge::fns());
    std_fns.extend(crate::elements::mouse_region::bridge::fns());
    std_fns.extend(crate::elements::condition::bridge::fns());
    std_fns.extend(crate::elements::switch::bridge::fns());
    std_fns.extend(crate::elements::each::bridge::fns());
    std_fns.extend(crate::elements::lazy_list::bridge::fns());
    std_fns.extend(crate::elements::scroll_view::bridge::fns());
    std_fns.extend(crate::elements::scrollbar::bridge::fns());
    std_fns.extend(crate::elements::fragment::bridge::fns());
    std_fns.extend(crate::elements::focusable::bridge::fns());
    std_fns.extend(crate::elements::effects::bridge::fns());
    std_fns.extend(crate::elements::lifecycle::bridge::fns());
    std_fns.extend(crate::elements::readable_subscribe::bridge::fns());

    let mut std_consts: Vec<ConstEntry> = Vec::new();
    std_consts.extend(color::consts(context, ctx_val.clone()));
    std_consts.extend(enums::consts(context));

    let std_module = module_loader::build_native_module(
        context,
        opaque.object().clone().into(),
        &std_fns,
        &std_consts,
    );
    loader.register("builtin:tur/std", std_module);

    // Register the public `turDevTool` global — a plain object whose methods
    // are the dev-tool natives bound to the bridge ctx (ctx-free surface).
    let dt_obj = JsObject::with_object_proto(context.intrinsics());
    let et_fn = module_loader::bound_native(
        context,
        ctx_val.clone(),
        dev_tool::tur_dev_tool_element_tree,
        0,
        "elementTree",
    );
    let ge_fn = module_loader::bound_native(
        context,
        ctx_val,
        dev_tool::tur_dev_tool_get_element,
        1,
        "getElement",
    );
    let _ = dt_obj.create_data_property(
        js_string!("elementTree"),
        boa_engine::JsValue::from(et_fn),
        context,
    );
    let _ = dt_obj.create_data_property(
        js_string!("getElement"),
        boa_engine::JsValue::from(ge_fn),
        context,
    );
    context
        .register_global_property(js_string!("turDevTool"), dt_obj, Attribute::all())
        .expect("failed to register turDevTool global");

    let schedule_flush = internal.needs_draw.clone();
    let timer_state = std::rc::Rc::new(std::cell::RefCell::new(TimerState::new()));
    timer::register_timer_globals(context, timer_state, schedule_flush);
    console::register_console_globals(context);

    tracing::info!("tur bridge initialized");

    BridgeResult { internal, executor }
}
