use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use tur_shared::{Alignment, Size, StackFit};

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::view::{ViewCx, val_from_js, Lifecycle, PropValue, View, Val};

// ---------------------------------------------------------------------------
// StackView — the user's declaration. Pure Rust, no JsValues.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct StackView {
    pub fit: Option<Val<StackFit>>,
    pub alignment: Option<Val<Alignment>>,
    pub children: Vec<Rc<dyn View>>,
    pub query_key: Option<Vec<String>>,
}

impl View for StackView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(StackElement {
                view: self.clone(),
                computed_size: None,
            }),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        for child_spec in &self.children {
            let _child_id = child_spec.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// StackElement — the built element. Layers its non-positioned children using
// `alignment`; children wrapped in `PositionedElement` (type name `tur_positioned`)
// place themselves.
// ---------------------------------------------------------------------------

pub struct StackElement {
    pub view: StackView,
    pub computed_size: Option<Size>,
}

impl Lifecycle for StackElement {}

impl ElementSubscribe for StackElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.fit.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.alignment.as_ref() { cx.subscribe_val(v); }
    }
}

impl ElementTrace for StackElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(Val::Static(f)) = &self.view.fit {
            parts.push(format!("fit={f:?}"));
        }
        if let Some(Val::Static(a)) = &self.view.alignment {
            parts.push(format!("alignment={a:?}"));
        }
        parts.join(" ")
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let c = &self.view;
        let mut p = Vec::new();
        if let Some(v) = c.fit.as_ref().and_then(Val::as_static) {
            p.push(("fit", TraceValue::Str(format!("{v:?}"))));
        }
        if let Some(v) = c.alignment.as_ref().and_then(Val::as_static) {
            p.push(("alignment", TraceValue::Str(format!("{v:?}"))));
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

/// Extract a `Val<T>` prop from a JS props object.
fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

/// Extract a `Vec<String>` prop (queryKey) — parsed eagerly.
fn prop_query_key(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Vec<String>> {
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

/// Extract child specs from a JS array of ComponentHandle opaques.
fn prop_children(props: &JsObject, key: &str, ctx: &mut Context) -> Vec<Rc<dyn View>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    use crate::core::view::extract_view;
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
            if let Some(spec) = extract_view(&item) {
                out.push(spec);
            }
        }
    }
    out
}

impl StackView {
    /// Build a `StackView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        StackView {
            fit: prop_val::<StackFit>(props, "fit", ctx),
            alignment: prop_val::<Alignment>(props, "alignment", ctx),
            children: prop_children(props, "children", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
