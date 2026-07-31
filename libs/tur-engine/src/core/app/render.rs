//! `render(ctx, rootViewHandle)` — mount the view tree into the ElementTree.

use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

use crate::core::app::root::RootView;
use crate::core::js_runtime::helpers::{FnEntry, Ptr, extract_ctx};
use crate::core::view::{SharedViewCx, View, extract_view};

pub fn fns() -> Vec<FnEntry> {
    vec![("render", 2, tur_render as Ptr)]
}

fn tur_render(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let user_view = extract_view(args.get_or_undefined(1)).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("render: expected a view handle as second argument"),
        )
    })?;

    // Wrap the user's view in the engine-owned `RootView` (`RootElement`).
    // The wrapper is mandatory: the user's view may be a fragment
    // (`Switch` / `Each` / `Condition` / `Fragment`) with no
    // `perform_layout` of its own, so the engine needs a layout-capable
    // element at the root. The wrapper is a minimal vertical-stack in
    // `core::app::root` — `core::app::render` has zero coupling to any
    // layout plugin (historical FlexView wrapper removed).
    let root_view = RootView { child: user_view };

    let mut cx = SharedViewCx::new(js_ctx.clone());
    let temp_parent = cx.alloc_node();
    let root_id = root_view.build(&mut cx, context, temp_parent);
    js_ctx
        .element_tree
        .borrow_mut()
        .set_root_element(crate::core::element::ElementNodeId::new(root_id.as_u64()));

    tracing::info!("render: view tree built");
    Ok(JsValue::undefined())
}
