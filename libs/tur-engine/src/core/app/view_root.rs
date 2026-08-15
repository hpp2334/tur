//! View-root bridge — `viewRoot` / `viewRoots` / `setViewRoot` /
//! `resetViewRoot` — the JS-facing mount API (replaces the former
//! `render(view)`).
//!
//! ```js
//! import { viewRoot, viewRoots, setViewRoot, resetViewRoot } from "tur:std";
//!
//! const main = viewRoot("main");     // opaque handle; throws on unknown name
//! setViewRoot(main, Shell);          // mount or replace (old subtree unmounted)
//! resetViewRoot(main);               // unmount + CLEAR mount intent
//! get(main.viewportSize$);           // per-root size atom
//! get(main.active$);                 // host-written lifecycle mirror
//! ```

use boa_engine::object::builtins::JsArray;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, js_string};

use crate::core::app::root::RootView;
use crate::core::app::view_roots::ViewRootHandle;
use crate::core::element::ViewRootId;
use crate::core::js_runtime::helpers::{FnEntry, Ptr, extract_js_ctx};
use crate::core::js_runtime::{BoaOpaque, TurInstanceContext};
use crate::core::view::{SharedViewCx, View, extract_view};

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("viewRoot", 2, tur_view_root as Ptr),
        ("viewRoots", 1, tur_view_roots as Ptr),
        ("setViewRoot", 3, tur_set_view_root as Ptr),
        ("resetViewRoot", 2, tur_reset_view_root as Ptr),
    ]
}

/// Build the JS handle object for one view root: a `ViewRootHandle` opaque
/// carrying the root id, with `name` / `viewportSize$` / `active$` data
/// properties. The atom handles are cloned from the registry slot so every
/// `viewRoot(name)` call returns the SAME atom identity (`get` works across
/// handles).
fn root_handle_js(
    js_ctx: &TurInstanceContext,
    root: ViewRootId,
    boa: &mut Context,
) -> JsResult<JsValue> {
    let (handle, name, viewport_js, active_js) = {
        let roots = js_ctx.view_roots.borrow();
        let slot = roots.get(root).ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message("view root no longer exists"))
        })?;
        (
            ViewRootHandle::new(slot.id, &slot.name),
            slot.name.clone(),
            slot.viewport_size_js.clone(),
            slot.active_js.clone(),
        )
    };
    let opaque = BoaOpaque::new(handle, boa);
    let obj = opaque.object().clone();
    let _ = obj.create_data_property(js_string!("name"), JsValue::from(js_string!(name)), boa);
    let _ = obj.create_data_property(js_string!("viewportSize$"), viewport_js, boa);
    let _ = obj.create_data_property(js_string!("active$"), active_js, boa);
    Ok(obj.into())
}

fn resolve_handle_arg(js_ctx: &TurInstanceContext, value: &JsValue) -> JsResult<ViewRootId> {
    let obj = value.as_object().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("expected a view-root handle (from viewRoot(name))"),
        )
    })?;
    let handle: boa_engine::object::Ref<'_, ViewRootHandle> =
        BoaOpaque::<ViewRootHandle>::wrap(&obj).ok_or_else(|| {
            JsError::from(
                JsNativeError::typ()
                    .with_message("expected a view-root handle (from viewRoot(name))"),
            )
        })?;
    let id = handle.id;
    drop(handle);
    if js_ctx.view_roots.borrow().get(id).is_none() {
        return Err(JsError::from(
            JsNativeError::typ().with_message("view root no longer exists"),
        ));
    }
    Ok(id)
}

/// `viewRoot(name)` → the opaque root handle.
fn tur_view_root(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let name = args
        .get_or_undefined(1)
        .as_string()
        .map(|s| s.to_std_string_escaped())
        .ok_or_else(|| {
            JsError::from(
                JsNativeError::typ().with_message("viewRoot: expected a root name string"),
            )
        })?;
    let root = js_ctx.view_roots.borrow().id_of(&name).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(format!(
            "viewRoot: unknown view root `{name}` (known roots are registered by the host at build time)"
        )))
    })?;
    root_handle_js(&js_ctx, root, context)
}

/// `viewRoots()` → array of registered root names, in registration order.
fn tur_view_roots(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let names = js_ctx.view_roots.borrow().names();
    let arr = JsArray::new(context)?;
    for name in names {
        arr.push(js_string!(name), context)?;
    }
    Ok(arr.into())
}

/// `setViewRoot(handle, view)` — mount (or replace) the root's view.
///
/// Replacing destroys the previous subtree first (unmount hooks fire on the
/// next flush, same machinery as a `Switch` branch swap). Mounting while the
/// root is torn down records the intent only — the build is deferred until
/// the host calls `setup_root`.
fn tur_set_view_root(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let root = resolve_handle_arg(&js_ctx, args.get_or_undefined(1))?;
    let user_view = extract_view(args.get_or_undefined(2)).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message("setViewRoot: expected a view handle as second argument"),
        )
    })?;

    set_view_root_impl(&js_ctx, root, user_view, context);
    Ok(JsValue::undefined())
}

pub(crate) fn set_view_root_impl(
    js_ctx: &TurInstanceContext,
    root: ViewRootId,
    user_view: std::rc::Rc<dyn View>,
    boa: &mut Context,
) {
    // Destroy any previously-built subtree (unmount hooks queue on the
    // tree's pending_destroy; the flush loop fires them). `destroy_subtree`
    // also clears the tree's root pointer.
    let (tree, setup) = {
        let mut roots = js_ctx.view_roots.borrow_mut();
        let slot = roots.get_mut(root).expect("resolve_handle_arg validated");
        if let Some(built) = slot.built_root.take() {
            slot.tree.destroy_subtree(built);
        }
        slot.mounted_handle = Some(user_view.clone());
        (slot.tree.clone(), slot.setup)
    };

    if setup {
        build_root(js_ctx, root, tree, user_view, boa);
    }
    // setViewRoot while torn-down: intent recorded; build deferred to setup.
}

/// Build `view` under a fresh `RootElement` in `tree` and register it as the
/// root element. Mirrors the historical `render()` mount path.
pub(crate) fn build_root(
    js_ctx: &TurInstanceContext,
    root: ViewRootId,
    tree: crate::core::elements::NodeTree,
    user_view: std::rc::Rc<dyn View>,
    boa: &mut Context,
) {
    // Wrap the user's view in the engine-owned `RootView` (`RootElement`).
    // The wrapper is mandatory: the user's view may be a fragment
    // (`Switch` / `Each` / `Condition` / `Fragment`) with no
    // `perform_layout` of its own, so the engine needs a layout-capable
    // element at the root.
    let root_view = RootView { child: user_view };

    let mut cx = SharedViewCx::for_tree(js_ctx.clone(), tree.clone());
    let temp_parent = cx.alloc_node();
    let root_id = root_view.build(&mut cx, boa, temp_parent);
    let root_element_id = root_id.as_element_id();
    tree.set_root_element(root_element_id);

    js_ctx
        .view_roots
        .borrow_mut()
        .get_mut(root)
        .expect("root slot exists")
        .built_root = Some(root_element_id);

    tracing::info!("setViewRoot: view tree built for root {root}");
}

/// `resetViewRoot(handle)` — unmount the root's built tree AND clear the
/// mount intent (a later host `setup_root` finds nothing to rebuild).
fn tur_reset_view_root(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let root = resolve_handle_arg(&js_ctx, args.get_or_undefined(1))?;
    reset_view_root_impl(&js_ctx, root);
    Ok(JsValue::undefined())
}

pub(crate) fn reset_view_root_impl(js_ctx: &TurInstanceContext, root: ViewRootId) {
    let mut roots = js_ctx.view_roots.borrow_mut();
    let slot = roots.get_mut(root).expect("resolve_handle_arg validated");
    if let Some(built) = slot.built_root.take() {
        slot.tree.destroy_subtree(built);
        js_ctx.set_dirty();
    }
    slot.mounted_handle = None;
}

/// Host-driven teardown: destroy the built tree but RETAIN the mount intent.
/// `WorkerMsg::TearDownRoot` calls this.
pub(crate) fn tear_down_root_impl(
    js_ctx: &TurInstanceContext,
    root: ViewRootId,
    boa: &mut Context,
) {
    let mut roots = js_ctx.view_roots.borrow_mut();
    let Some(slot) = roots.get_mut(root) else {
        return;
    };
    if !slot.setup {
        return; // idempotent
    }
    slot.setup = false;
    if let Some(built) = slot.built_root.take() {
        slot.tree.destroy_subtree(built);
        js_ctx.set_dirty();
    }
    roots.set_active(root, false, boa);
    tracing::info!("view root {root} torn down (mount intent retained)");
}

/// Host-driven setup: rebuild from the retained mount intent (if any).
/// `WorkerMsg::SetupRoot` calls this.
pub(crate) fn setup_root_impl(js_ctx: &TurInstanceContext, root: ViewRootId, boa: &mut Context) {
    let (tree, intent) = {
        let mut roots = js_ctx.view_roots.borrow_mut();
        let Some(slot) = roots.get_mut(root) else {
            return;
        };
        if slot.setup {
            return; // idempotent
        }
        slot.setup = true;
        let tree = slot.tree.clone();
        let intent = slot.mounted_handle.clone();
        roots.set_active(root, true, boa);
        (tree, intent)
    };
    if let Some(view) = intent {
        build_root(js_ctx, root, tree, view, boa);
        js_ctx.request_paint();
    }
    tracing::info!("view root {root} set up");
}
