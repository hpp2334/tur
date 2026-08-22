//! `mount(view)` — mount the view tree's ROOT into the instance-owned
//! `ElementTree`, building against the tree's permanently-bound INSTANCE
//! store (the engine-created store handed to the module's `start({ store })`).
//! Every declaration the tree touches (props, subscriptions, closures)
//! materializes into that store's KV.

use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

use crate::core::app::root::RootView;
use crate::core::js_runtime::helpers::{FnEntry, Ptr, extract_js_ctx};
use crate::core::view::{SharedViewCx, View, extract_view};

pub fn fns() -> Vec<FnEntry> {
    vec![("mount", 1, tur_mount as Ptr)]
}

fn tur_mount(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let user_view = extract_view(args.get_or_undefined(1)).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("mount: expected a view handle — mount(view)"),
        )
    })?;

    let tree = js_ctx.element_tree.clone();

    // One-root invariant: tear down any root a previous `mount` left behind
    // (a re-mount replaces the root rather than leaking the old subtree).
    // This is also what cleans up on the module-lifecycle teardown path, so
    // a module's cleanup never needs to unmount its own tree — the engine
    // owns root-tree lifecycle.
    {
        let old_root = tree.borrow().root_element_id();
        if let Some(old) = old_root {
            tracing::debug!("mount: replacing existing root {old:?}");
            tree.borrow_mut().destroy_subtree(old);
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
    tree.borrow_mut()
        .set_root_element(crate::core::element::ElementNodeId::new(root_id.as_u64()));

    tracing::info!("mount: view tree built");
    Ok(JsValue::undefined())
}
