use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use tur_shared::Axis;

use crate::core::bridge::utils::extract_ctx;
use crate::core::view::{View, ViewHandle, SharedViewCx};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_props_object(args: &[JsValue], idx: usize, ctx: &mut Context) -> JsResult<JsObject> {
    let v = args.get_or_undefined(idx);
    if v.is_undefined() || v.is_null() {
        let proto = ctx.intrinsics().constructors().object().prototype();
        return Ok(JsObject::from_proto_and_data(proto, ()));
    }
    let obj = v.as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected props object"))
    })?;
    Ok(obj.clone())
}

fn wrap_view(spec: Rc<dyn View>, context: &mut Context) -> JsValue {
    let opaque = crate::core::bridge::BoaOpaque::new(ViewHandle::new(spec), context);
    opaque.object().clone().into()
}

// ---------------------------------------------------------------------------
// View spec factories
// ---------------------------------------------------------------------------

macro_rules! spec_factory {
    ($fn_name:ident, $spec_ty:ident) => {
        pub(crate) fn $fn_name(
            _this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            let _ = extract_ctx(args)?;
            let props = require_props_object(args, 1, context)?;
            let spec = crate::elements::$spec_ty::from_js(&props, context);
            Ok(wrap_view(Rc::new(spec), context))
        }
    };
}

spec_factory!(tur_container, ContainerView);
spec_factory!(tur_text, TextView);
spec_factory!(tur_stack, StackView);
spec_factory!(tur_image_edgy, ImageView);
spec_factory!(tur_condition, ConditionView);
spec_factory!(tur_switch, SwitchView);
spec_factory!(tur_input_edgy, InputView);
spec_factory!(tur_fragment, FragmentView);
spec_factory!(tur_pointer_interact, PointerInteractView);
spec_factory!(tur_focusable, FocusableView);
spec_factory!(tur_scrollbar, ScrollbarView);
spec_factory!(tur_mouse_region, MouseRegionView);
spec_factory!(tur_opacity, OpacityView);
spec_factory!(tur_transform, TransformView);

macro_rules! spec_factory_opt {
    ($fn_name:ident, $spec_ty:ident) => {
        pub(crate) fn $fn_name(
            _this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            let _ = extract_ctx(args)?;
            let props = require_props_object(args, 1, context)?;
            let spec = crate::elements::$spec_ty::from_js(&props, context)
                .ok_or_else(|| {
                    JsError::from(JsNativeError::typ()
                        .with_message(concat!("missing required prop for ", stringify!($spec_ty))))
                })?;
            Ok(wrap_view(Rc::new(spec), context))
        }
    };
}

spec_factory_opt!(tur_expanded, ExpandedView);
spec_factory_opt!(tur_positioned, PositionedView);
spec_factory_opt!(tur_scroll_view, ScrollViewView);
spec_factory_opt!(tur_lazy_list, LazyListView);
spec_factory_opt!(tur_each, EachView);

pub(crate) fn tur_column(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = crate::elements::FlexView::from_js(Axis::Vertical, &props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

pub(crate) fn tur_row(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = crate::elements::FlexView::from_js(Axis::Horizontal, &props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

// ---------------------------------------------------------------------------
// render(ctx, rootViewHandle) — mount the view tree into ElementTree
// ---------------------------------------------------------------------------

pub(crate) fn tur_render(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let user_view = crate::core::view::extract_view(args.get_or_undefined(1))
        .ok_or_else(|| {
            JsError::from(
                JsNativeError::typ()
                    .with_message("render: expected a view handle as second argument"),
            )
        })?;

    // Wrap the user's view in a root flex container so the tree always has
    // a stable root node (tests expect root.kind == "tur_flex"). The user
    // view is typically a `JsView` whose `build()` invokes the JS
    // thunk to produce the real subtree.
    let root_view = crate::elements::FlexView {
        direction: Some(tur_shared::Axis::Vertical),
        main_alignment: None,
        cross_alignment: None,
        main_axis_size: None,
        children: vec![user_view],
        query_key: None,
    };

    let mut cx = SharedViewCx::new(js_ctx.clone());
    let temp_parent = cx.alloc_node();
    let root_id = root_view.build(&mut cx, context, temp_parent);
    js_ctx.element_tree.borrow_mut().set_root_element(crate::core::element::ElementNodeId::new(root_id.as_u64()));

    tracing::info!("render: view tree built");
    Ok(JsValue::undefined())
}
