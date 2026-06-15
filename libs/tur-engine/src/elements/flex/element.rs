use std::rc::Rc;

use boa_engine::Context;
use tur_shared::{Axis, Constraints, CrossAxisAlignment, MainAxisSize, MainAxisAlignment, Size};

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::widget::{val_from_js, Effect, PropValue, Spec, Val, WidgetCx};

pub(crate) struct ChildData {
    pub id: ElementNodeId,
    pub size: Size,
    pub is_flex: bool,
}

// ---------------------------------------------------------------------------
// FlexSpec — the user's declaration. Pure Rust, no JsValues.
//
// `direction` is static (chosen by the factory: Vertical for Column, Horizontal
// for Row) and therefore stored as a plain `Axis` rather than a `Val<Axis>`.
// The alignment / sizing props are reactive.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FlexSpec {
    pub direction: Option<Axis>,
    pub main_alignment: Option<Val<MainAxisAlignment>>,
    pub cross_alignment: Option<Val<CrossAxisAlignment>>,
    pub main_axis_size: Option<Val<MainAxisSize>>,
    pub children: Vec<Rc<dyn Spec>>,
    pub query_key: Option<Vec<String>>,
}

impl Spec for FlexSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::new(Flex {
                spec: self.clone(),
                child_data: Vec::new(),
                constraints: None,
                computed_size: None,
            }),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        for child_spec in &self.children {
            let _child_id = child_spec.build(cx, boa, id);
        }
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// Flex — the built element. Holds its spec plus transient layout state that
// must flow from `perform_layout_size` to `perform_layout_position`.
// ---------------------------------------------------------------------------

pub struct Flex {
    pub spec: FlexSpec,
    pub(crate) child_data: Vec<ChildData>,
    pub(crate) constraints: Option<Constraints>,
    pub(crate) computed_size: Option<Size>,
}

impl Effect for Flex {}

impl ElementTrace for Flex {
    fn trace_label(&self) -> String {
        format!("{:?}", self.spec.direction.unwrap_or(Axis::Vertical))
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

/// Extract a `Val<T>` prop from a JS props object.
fn prop_val<T: PropValue>(
    props: &boa_engine::object::JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

/// Extract a `Vec<String>` prop (queryKey) — parsed eagerly.
fn prop_query_key(
    props: &boa_engine::object::JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<Vec<String>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    let obj = v.as_object()?;
    let arr = JsArray::from_object(obj.clone()).ok()?;
    let len = arr.length(ctx).ok()? as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if let Ok(val) = arr.at(i as i64, ctx) {
            if let Some(s) = val.as_string() {
                out.push(s.to_std_string_escaped());
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Extract child specs from a JS array of SpecHandle opaques.
fn prop_children(
    props: &boa_engine::object::JsObject,
    key: &str,
    ctx: &mut Context,
) -> Vec<Rc<dyn Spec>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    use crate::core::widget::extract_spec;
    let Ok(v) = props.get(js_string!(key), ctx) else {
        return Vec::new();
    };
    let Some(obj) = v.as_object() else {
        return Vec::new();
    };
    let Ok(arr) = JsArray::from_object(obj.clone()) else {
        return Vec::new();
    };
    let len = arr.length(ctx).unwrap_or(0);
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
        if let Ok(item) = arr.at(i as i64, ctx) {
            if let Some(spec) = extract_spec(&item) {
                out.push(spec);
            }
        }
    }
    out
}

impl FlexSpec {
    /// Build a `FlexSpec` from a JS props object. `direction` is supplied by
    /// the factory (`Axis::Vertical` for Column, `Axis::Horizontal` for Row).
    pub fn from_js(direction: Axis, props: &boa_engine::object::JsObject, ctx: &mut Context) -> Self {
        FlexSpec {
            direction: Some(direction),
            main_alignment: prop_val::<MainAxisAlignment>(props, "mainAlignment", ctx),
            cross_alignment: prop_val::<CrossAxisAlignment>(props, "crossAlignment", ctx),
            main_axis_size: prop_val::<MainAxisSize>(props, "mainAxisSize", ctx),
            children: prop_children(props, "children", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
