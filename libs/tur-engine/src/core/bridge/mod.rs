use boa_engine::context::time::FixedClock;
use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::Context;

use crate::core::app::TurAppInternal;
use crate::core::bridge::color::{
    tur_color_hex, tur_color_lerp, tur_color_rgb, tur_color_rgba, tur_create_color,
    tur_create_linear_gradient, tur_linear_gradient_create,
};
use crate::core::bridge::dev_tool::{tur_dev_tool_element_tree, tur_dev_tool_get_element};
use crate::core::bridge::reactive_bridge::{
    tur_view, tur_derive, tur_get, tur_mutate, tur_set, tur_source,
};
use crate::core::bridge::utils::{
    tur_create_animation_controller, tur_create_image_resource, tur_create_lazy_list_controller,
    tur_create_scroll_controller, tur_create_svg_resource, tur_create_text_editing_controller,
    tur_create_undo_controller, tur_request_focus,
};
use crate::core::bridge::view_bridge::{
    tur_column, tur_condition, tur_container, tur_expanded, tur_focusable, tur_each,
    tur_fragment, tur_image_edgy, tur_input_edgy, tur_lazy_list, tur_lifecycle_view,
    tur_mouse_region, tur_readable_subscribe, tur_switch, tur_opacity, tur_pointer_interact,
    tur_positioned, tur_render, tur_row, tur_scroll_view, tur_scrollbar, tur_stack, tur_text,
    tur_transform,
};
use crate::core::fonts::FontLoader;
use crate::core::render::Renderer;

pub(crate) mod color;
pub(crate) mod console;
pub(crate) mod dev_tool;
pub(crate) mod executor;
pub(crate) mod module_loader;
pub(crate) mod reactive_bridge;
pub(crate) mod utils;
pub(crate) mod view_bridge;
mod js_context;
mod opaque;
pub(crate) mod timer;

pub use executor::TurJobExecutor;
pub use console::register_console_globals;
pub use js_context::TurJsContext;
pub use module_loader::TurModuleLoader;
pub use opaque::BoaOpaque;
pub use timer::TimerState;
pub use utils::TurNodeHandle;

/// Build a JS object modelling a TypeScript numeric `enum`: forward mapping
/// (`Vertical: 0`) AND reverse mapping (`"0": "Vertical"`), exactly as `tsc`
/// emits for `enum Axis { Vertical, Horizontal }`. Exported from the
/// `builtin:tur/core` module so JS callers write `Axis.Vertical` and
/// `Axis[0] === "Vertical"`. Members + values mirror `tur_shared` C-like enums.
fn build_enum(context: &mut Context, pairs: &[(&str, u32)]) -> boa_engine::JsValue {
    let obj = JsObject::with_object_proto(context.intrinsics());
    for (name, val) in pairs {
        // forward: name -> number
        let _ = obj.create_data_property(
            js_string!(*name),
            boa_engine::JsValue::from(*val as f64),
            context,
        );
        // reverse: number (as string key) -> name
        let _ = obj.create_data_property(
            js_string!(val.to_string()),
            boa_engine::JsValue::from(js_string!(*name)),
            context,
        );
    }
    obj.into()
}

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

       let fns: [(
        &str,
        usize,
        boa_engine::native_function::NativeFunctionPointer,
    ); 34] = [
        // --- reactive primitives ---
        ("source", 2, tur_source),
        ("derive", 2, tur_derive),
        ("mutate", 2, tur_mutate),
        ("get", 2, tur_get),
        ("set", 3, tur_set),
        ("view", 1, tur_view),
        // --- view spec factories ---
        ("Container", 2, tur_container),
        ("Column", 2, tur_column),
        ("Row", 2, tur_row),
        ("Expanded", 2, tur_expanded),
        ("Stack", 2, tur_stack),
        ("Positioned", 2, tur_positioned),
        ("Text", 2, tur_text),
        ("PointerInteract", 2, tur_pointer_interact),
        ("MouseRegion", 2, tur_mouse_region),
        ("Condition", 2, tur_condition),
        ("Switch", 2, tur_switch),
        ("Each", 2, tur_each),
        ("LazyList", 2, tur_lazy_list),
        ("ScrollView", 2, tur_scroll_view),
        ("Scrollbar", 2, tur_scrollbar),
        ("ImageEdgy", 2, tur_image_edgy),
        ("InputEdgy", 2, tur_input_edgy),
        ("Fragment", 2, tur_fragment),
        ("Focusable", 2, tur_focusable),
        ("Opacity", 2, tur_opacity),
        ("Transform", 2, tur_transform),
        ("lifecycleView", 1, tur_lifecycle_view),
        ("ReadableSubscribe", 2, tur_readable_subscribe),
        ("render", 2, tur_render),
        // --- utility functions ---
        ("createTextEditingController", 2, tur_create_text_editing_controller),
        ("createUndoController", 2, tur_create_undo_controller),
        ("createScrollController", 2, tur_create_scroll_controller),
        ("createLazyListController", 2, tur_create_lazy_list_controller),
    ];

    // Collect the full public bridge table (fixed array + variadic helpers)
    // into a single list. The `builtin:tur/core` module is built from this.
    type Ptr = boa_engine::native_function::NativeFunctionPointer;
    let mut all_fns: Vec<(&str, usize, Ptr)> = fns.to_vec();
    all_fns.extend([
        ("createColor", 4, tur_create_color as Ptr),
        ("colorLerp", 3, tur_color_lerp as Ptr),
        ("createLinearGradient", 5, tur_create_linear_gradient as Ptr),
        ("createAnimationController", 2, tur_create_animation_controller as Ptr),
        ("createImageResource", 2, tur_create_image_resource as Ptr),
        ("createSvgResource", 2, tur_create_svg_resource as Ptr),
        ("requestFocus", 2, tur_request_focus as Ptr),
        // `SizedBox` is a width/height-only `Container` — same native fn,
        // exported under an alias so JS callers write `SizedBox({width,height})`.
        ("SizedBox", 2, tur_container as Ptr),
    ]);

    let internal = TurAppInternal::new(renderer, font_loader, executor.clone(), clock, platform_api);
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
        ctx.register_handler(Box::new(
            crate::handlers::scroll_to::ScrollToHandler,
        ));
        ctx.register_handler(Box::new(
            crate::handlers::clipboard::ClipboardPasteHandler,
        ));
        ctx.register_handler(Box::new(
            crate::handlers::clipboard::ClipboardWriteHandler,
        ));
    }

    let opaque = BoaOpaque::new(internal.js_context.clone(), context);

    // Build the `builtin:tur/core` module from the full bridge table, binding each
    // native fn to the bridge ctx so JS callers get a ctx-free surface. Enum
    // objects (Axis, MainAxisSize, …) and the `Color` / `LinearGradient`
    // builder const-objects are exported as constant values.
    use boa_engine::JsValue as JsVal;
    let ctx_val: JsVal = opaque.object().clone().into();

    // `Color` — a namespace of static builders (`Color.hex/rgb/rgba`). Each
    // method is a bound native forwarding to the color bridge fns; the runtime
    // values returned are `ColorOpaque` handles (the `Color` *instance* type
    // in the TS decls). Users never `new Color()`.
    let color_obj = JsObject::with_object_proto(context.intrinsics());
    let _ = color_obj.create_data_property(
        js_string!("rgb"),
        JsVal::from(module_loader::bound_native(
            context,
            ctx_val.clone(),
            tur_color_rgb,
            3,
            "rgb",
        )),
        context,
    );
    let _ = color_obj.create_data_property(
        js_string!("rgba"),
        JsVal::from(module_loader::bound_native(
            context,
            ctx_val.clone(),
            tur_color_rgba,
            4,
            "rgba",
        )),
        context,
    );
    let _ = color_obj.create_data_property(
        js_string!("hex"),
        JsVal::from(module_loader::bound_native(
            context,
            ctx_val.clone(),
            tur_color_hex,
            1,
            "hex",
        )),
        context,
    );

    // `LinearGradient` — namespace with a single `create(options)` builder.
    let linear_obj = JsObject::with_object_proto(context.intrinsics());
    let _ = linear_obj.create_data_property(
        js_string!("create"),
        JsVal::from(module_loader::bound_native(
            context,
            ctx_val.clone(),
            tur_linear_gradient_create,
            1,
            "create",
        )),
        context,
    );

    let consts: Vec<(&str, JsVal)> = vec![
        ("Axis", build_enum(context, &[("Vertical", 0), ("Horizontal", 1)])),
        (
            "MainAxisAlignment",
            build_enum(
                context,
                &[
                    ("Start", 0),
                    ("Center", 1),
                    ("End", 2),
                    ("SpaceBetween", 3),
                    ("SpaceAround", 4),
                    ("SpaceEvenly", 5),
                ],
            ),
        ),
        (
            "CrossAxisAlignment",
            build_enum(
                context,
                &[("Start", 0), ("Center", 1), ("End", 2), ("Stretch", 3)],
            ),
        ),
        ("MainAxisSize", build_enum(context, &[("Max", 0), ("Min", 1)])),
        (
            "HitTestBehavior",
            build_enum(context, &[("Opaque", 0), ("Translucent", 1)]),
        ),
        (
            "BoxFit",
            build_enum(
                context,
                &[
                    ("Fill", 0),
                    ("Contain", 1),
                    ("Cover", 2),
                    ("FitWidth", 3),
                    ("FitHeight", 4),
                    ("None", 5),
                ],
            ),
        ),
        (
            "BorderPosition",
            build_enum(context, &[("Inside", 0), ("Center", 1), ("Outside", 2)]),
        ),
        (
            "Alignment",
            build_enum(
                context,
                &[
                    ("TopLeft", 0),
                    ("TopCenter", 1),
                    ("TopRight", 2),
                    ("CenterLeft", 3),
                    ("Center", 4),
                    ("CenterRight", 5),
                    ("BottomLeft", 6),
                    ("BottomCenter", 7),
                    ("BottomRight", 8),
                ],
            ),
        ),
        ("Color", color_obj.into()),
        ("LinearGradient", linear_obj.into()),
    ];
    let core_module = module_loader::build_native_module(
        context,
        opaque.object().clone().into(),
        &all_fns,
        &consts,
    );
    loader.register("builtin:tur/core", core_module);

    // Register the public `turDevTool` global — a plain object whose methods
    // are the dev-tool natives bound to the bridge ctx (ctx-free surface).
    // Built directly (no `__tur` intermediary): `{ elementTree(), getElement(id) }`.
    let dt_obj = JsObject::with_object_proto(context.intrinsics());
    let et_fn = module_loader::bound_native(
        context,
        ctx_val.clone(),
        tur_dev_tool_element_tree,
        0,
        "elementTree",
    );
    let ge_fn = module_loader::bound_native(
        context,
        ctx_val,
        tur_dev_tool_get_element,
        1,
        "getElement",
    );
    let _ = dt_obj.create_data_property(js_string!("elementTree"), boa_engine::JsValue::from(et_fn), context);
    let _ = dt_obj.create_data_property(js_string!("getElement"), boa_engine::JsValue::from(ge_fn), context);
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
