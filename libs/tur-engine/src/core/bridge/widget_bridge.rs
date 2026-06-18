use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use tur_shared::Axis;

use crate::core::bridge::utils::extract_ctx;
use crate::core::widget::{Spec, SpecHandle, WidgetCx};

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

fn wrap_spec(spec: Rc<dyn Spec>, context: &mut Context) -> JsValue {
    let opaque = crate::core::bridge::BoaOpaque::new(SpecHandle::new(spec), context);
    opaque.object().clone().into()
}

// ---------------------------------------------------------------------------
// Widget spec factories
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
            Ok(wrap_spec(Rc::new(spec), context))
        }
    };
}

spec_factory!(tur_container, ContainerSpec);
spec_factory!(tur_text, TextSpec);
spec_factory!(tur_stack, StackSpec);
spec_factory!(tur_image_edgy, ImageSpec);
spec_factory!(tur_condition, ConditionSpec);
spec_factory!(tur_switch, SwitchSpec);
spec_factory!(tur_input_edgy, InputSpec);
spec_factory!(tur_fragment, FragmentSpec);
spec_factory!(tur_pointer_interact, PointerInteractSpec);
spec_factory!(tur_svg_edgy, SvgSpec);
spec_factory!(tur_focusable, FocusableSpec);

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
            Ok(wrap_spec(Rc::new(spec), context))
        }
    };
}

spec_factory_opt!(tur_expanded, ExpandedSpec);
spec_factory_opt!(tur_positioned, PositionedSpec);
spec_factory_opt!(tur_scroll_view, ScrollViewSpec);
spec_factory_opt!(tur_lazy_list, LazyListSpec);
spec_factory_opt!(tur_each, EachSpec);

pub(crate) fn tur_column(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = crate::elements::FlexSpec::from_js(Axis::Vertical, &props, context);
    Ok(wrap_spec(Rc::new(spec), context))
}

pub(crate) fn tur_row(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = crate::elements::FlexSpec::from_js(Axis::Horizontal, &props, context);
    Ok(wrap_spec(Rc::new(spec), context))
}

// ---------------------------------------------------------------------------
// render(ctx, rootSpecHandle) — mount the spec tree into ElementTree
// ---------------------------------------------------------------------------

pub(crate) fn tur_render(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let user_spec = crate::core::widget::extract_spec(args.get_or_undefined(1))
        .ok_or_else(|| {
            JsError::from(
                JsNativeError::typ()
                    .with_message("render: expected a spec handle as second argument"),
            )
        })?;

    // Wrap the user's spec in a root flex container so the tree always has
    // a stable root node (tests expect root.kind == "tur_flex").
    let root_spec = crate::elements::FlexSpec {
        direction: Some(tur_shared::Axis::Vertical),
        main_alignment: None,
        cross_alignment: None,
        main_axis_size: None,
        children: vec![user_spec],
        query_key: None,
    };

    let mut cx = WidgetCx::new(js_ctx.clone());
    let temp_parent = cx.alloc_node();
    let root_id = root_spec.build(&mut cx, context, temp_parent);
    {
        let mut tree = js_ctx.element_tree.borrow_mut();
        tree.set_root(root_id);
    }

    tracing::info!("render: spec tree built");
    Ok(JsValue::undefined())
}
