use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::property::Attribute;
use boa_engine::Context;
use tracing;
use tur_widget::WidgetTree;

use crate::widget_bridge::{
    tur_append_child, tur_create_element, tur_create_root, tur_get_first_child,
    tur_get_next_sibling, tur_get_parent, tur_insert_before, tur_remove_child, tur_set_attribute,
    TurAppContext,
};
use crate::BoaOpaque;

fn register_global_fn(
    context: &mut Context,
    name: &boa_engine::JsString,
    length: usize,
    f: boa_engine::native_function::NativeFunctionPointer,
) {
    let func = boa_engine::object::FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(f),
    )
    .name(name.clone())
    .length(length)
    .build();

    context
        .register_global_property(name.clone(), func, Attribute::all())
        .expect("failed to register global function");
}

pub fn init_bridge(context: &mut Context) -> (BoaOpaque<TurAppContext>, Rc<RefCell<WidgetTree>>) {
    register_global_fn(
        context,
        &js_string!("tur_createElement"),
        2,
        tur_create_element,
    );
    register_global_fn(context, &js_string!("tur_createRoot"), 1, tur_create_root);
    register_global_fn(
        context,
        &js_string!("tur_setAttribute"),
        4,
        tur_set_attribute,
    );
    register_global_fn(context, &js_string!("tur_appendChild"), 3, tur_append_child);
    register_global_fn(context, &js_string!("tur_removeChild"), 3, tur_remove_child);
    register_global_fn(
        context,
        &js_string!("tur_insertBefore"),
        4,
        tur_insert_before,
    );
    register_global_fn(context, &js_string!("tur_getParent"), 2, tur_get_parent);
    register_global_fn(
        context,
        &js_string!("tur_getFirstChild"),
        2,
        tur_get_first_child,
    );
    register_global_fn(
        context,
        &js_string!("tur_getNextSibling"),
        2,
        tur_get_next_sibling,
    );

    let ctx = TurAppContext::new();
    let tree_rc = ctx.tree_rc().clone();
    let opaque = BoaOpaque::new(ctx, context);

    tracing::info!("tur bridge initialized");

    (opaque, tree_rc)
}
