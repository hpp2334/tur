use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use tur_shared::Axis;

use crate::core::bridge::utils::extract_ctx;
use crate::core::widget::{Component, ComponentHandle, WidgetCx};

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

fn wrap_component(spec: Rc<dyn Component>, context: &mut Context) -> JsValue {
    let opaque = crate::core::bridge::BoaOpaque::new(ComponentHandle::new(spec), context);
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
            Ok(wrap_component(Rc::new(spec), context))
        }
    };
}

spec_factory!(tur_container, ContainerComponent);
spec_factory!(tur_text, TextComponent);
spec_factory!(tur_stack, StackComponent);
spec_factory!(tur_image_edgy, ImageComponent);
spec_factory!(tur_condition, ConditionComponent);
spec_factory!(tur_switch, SwitchComponent);
spec_factory!(tur_input_edgy, InputComponent);
spec_factory!(tur_fragment, FragmentComponent);
spec_factory!(tur_pointer_interact, PointerInteractComponent);
spec_factory!(tur_svg_edgy, SvgComponent);
spec_factory!(tur_focusable, FocusableComponent);
spec_factory!(tur_scrollbar, ScrollbarComponent);
spec_factory!(tur_mouse_region, MouseRegionComponent);

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
            Ok(wrap_component(Rc::new(spec), context))
        }
    };
}

spec_factory_opt!(tur_expanded, ExpandedComponent);
spec_factory_opt!(tur_positioned, PositionedComponent);
spec_factory_opt!(tur_scroll_view, ScrollViewComponent);
spec_factory_opt!(tur_lazy_list, LazyListComponent);
spec_factory_opt!(tur_each, EachComponent);

pub(crate) fn tur_column(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = crate::elements::FlexComponent::from_js(Axis::Vertical, &props, context);
    Ok(wrap_component(Rc::new(spec), context))
}

pub(crate) fn tur_row(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = crate::elements::FlexComponent::from_js(Axis::Horizontal, &props, context);
    Ok(wrap_component(Rc::new(spec), context))
}

// ---------------------------------------------------------------------------
// render(ctx, rootComponentHandle) — mount the component tree into ElementTree
// ---------------------------------------------------------------------------

pub(crate) fn tur_render(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let user_component = crate::core::widget::extract_component(args.get_or_undefined(1))
        .ok_or_else(|| {
            JsError::from(
                JsNativeError::typ()
                    .with_message("render: expected a component handle as second argument"),
            )
        })?;

    // Wrap the user's component in a root flex container so the tree always has
    // a stable root node (tests expect root.kind == "tur_flex"). The user
    // component is typically a `JsComponent` whose `build()` invokes the JS
    // thunk to produce the real subtree.
    let root_component = crate::elements::FlexComponent {
        direction: Some(tur_shared::Axis::Vertical),
        main_alignment: None,
        cross_alignment: None,
        main_axis_size: None,
        children: vec![user_component],
        query_key: None,
    };

    let mut cx = WidgetCx::new(js_ctx.clone());
    let temp_parent = cx.alloc_node();
    let root_id = root_component.build(&mut cx, context, temp_parent);
    js_ctx.element_tree.borrow_mut().set_root(root_id);

    tracing::info!("render: component tree built");
    Ok(JsValue::undefined())
}
