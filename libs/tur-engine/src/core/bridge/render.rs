//! `render(ctx, rootViewHandle)` — mount the view tree into the ElementTree.

use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

use crate::core::bridge::helpers::{extract_ctx, FnEntry, Ptr};
use crate::core::view::{extract_view, SharedViewCx, View};

pub fn fns() -> Vec<FnEntry> {
    vec![("render", 2, tur_render as Ptr)]
}

fn tur_render(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let user_view = extract_view(args.get_or_undefined(1)).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("render: expected a view handle as second argument"),
        )
    })?;

    // Wrap the user's view in a root flex container so the tree always has
    // a stable root node (tests expect root.kind == "tur_flex"). The user
    // view is typically a `JsView` whose `build()` invokes the JS
    // thunk to produce the real subtree.
    let root_view = crate::elements::FlexView {
        direction: Some(crate::core::layout::Axis::Vertical),
        main_alignment: None,
        cross_alignment: None,
        main_axis_size: None,
        children: vec![user_view],
        query_key: None,
    };

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
