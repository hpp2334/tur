use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use tur_shared::{Alignment, Size, StackFit};

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::widget::{val_from_js, Effect, PropValue, Component, Val, WidgetCx};

// ---------------------------------------------------------------------------
// StackComponent — the user's declaration. Pure Rust, no JsValues.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct StackComponent {
    pub fit: Option<Val<StackFit>>,
    pub alignment: Option<Val<Alignment>>,
    pub children: Vec<Rc<dyn Component>>,
    pub query_key: Option<Vec<String>>,
}

impl Component for StackComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::new(StackElement {
                component: self.clone(),
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
// StackElement — the built element. Layers its non-positioned children using
// `alignment`; children wrapped in `PositionedElement` (type name `tur_positioned`)
// place themselves.
// ---------------------------------------------------------------------------

pub struct StackElement {
    pub component: StackComponent,
    pub(crate) computed_size: Option<Size>,
}

impl Effect for StackElement {}

impl ElementTrace for StackElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(Val::Static(f)) = &self.component.fit {
            parts.push(format!("fit={f:?}"));
        }
        if let Some(Val::Static(a)) = &self.component.alignment {
            parts.push(format!("alignment={a:?}"));
        }
        parts.join(" ")
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
fn prop_children(props: &JsObject, key: &str, ctx: &mut Context) -> Vec<Rc<dyn Component>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    use crate::core::widget::extract_component;
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
            if let Some(spec) = extract_component(&item) {
                out.push(spec);
            }
        }
    }
    out
}

impl StackComponent {
    /// Build a `StackComponent` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        StackComponent {
            fit: prop_val::<StackFit>(props, "fit", ctx),
            alignment: prop_val::<Alignment>(props, "alignment", ctx),
            children: prop_children(props, "children", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
