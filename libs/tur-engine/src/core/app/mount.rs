//! `mount(ctx, store, rootViewHandle)` — mount the view tree into the
//! ElementTree, binding `store` as the tree's mounted store (declarations in
//! the tree materialize into that store's KV).

use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

use crate::core::app::root::RootView;
use crate::core::edgy::reactive::extract_store;
use crate::core::js_runtime::helpers::{FnEntry, Ptr, extract_js_ctx};
use crate::core::view::{SharedViewCx, View, extract_view};

pub fn fns() -> Vec<FnEntry> {
    vec![("mount", 3, tur_mount as Ptr)]
}

fn tur_mount(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let store = extract_store(args.get_or_undefined(1)).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message("mount: expected a store as second argument (createStore())"),
        )
    })?;
    if !js_ctx.store.same_instance(&store) {
        return Err(JsError::from(JsNativeError::typ().with_message(
            "mount: the store belongs to a different instance — \
                 use a store from createStore() in this module",
        )));
    }
    let user_view = extract_view(args.get_or_undefined(2)).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("mount: expected a view handle as third argument"),
        )
    })?;

    // Bind the tree's mounted store BEFORE building, so every declaration
    // touched by the tree (props, subscriptions, closures) materializes into
    // this store. All stores share the instance's reactive machinery, so
    // atoms of a previous mount keep routing correctly.
    js_ctx.element_tree.set_store(store);

    // One-root invariant: tear down any tree a previous `mount` left behind
    // (a re-mount replaces the root rather than leaking the old subtree).
    // This is also what cleans up on the module-lifecycle teardown path, so
    // a module's cleanup never needs to unmount its own tree — the engine
    // owns root-tree lifecycle.
    {
        let js = &js_ctx;
        let old_root = js.element_tree.borrow().root_element_id();
        if let Some(old) = old_root {
            tracing::debug!("mount: replacing existing root {old:?}");
            js.element_tree.borrow_mut().destroy_subtree(old);
        }
    }

    // Wrap the user's view in the engine-owned `RootView` (`RootElement`).
    // The wrapper is mandatory: the user's view may be a fragment
    // (`Switch` / `Each` / `Condition` / `Fragment`) with no
    // `perform_layout` of its own, so the engine needs a layout-capable
    // element at the root. The wrapper is a minimal vertical-stack in
    // `core::app::root` — `core::app::mount` has zero coupling to any
    // layout plugin (historical FlexView wrapper removed).
    let root_view = RootView { child: user_view };

    let mut cx = SharedViewCx::new(js_ctx.clone());
    let temp_parent = cx.alloc_node();
    let root_id = root_view.build(&mut cx, context, temp_parent);
    js_ctx
        .element_tree
        .borrow_mut()
        .set_root_element(crate::core::element::ElementNodeId::new(root_id.as_u64()));

    tracing::info!("mount: view tree built");
    Ok(JsValue::undefined())
}
