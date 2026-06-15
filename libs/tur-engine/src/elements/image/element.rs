use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use tur_shared::BoxFit;

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::widget::{
    extract_spec, val_from_js, Effect, PropValue, Spec, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// ImageSpec — the user's declaration. Pure Rust, no JsValues.
//
// `resource_id`, `width`, `height`, and `fit` are reactive (`Val<T>`).
// An optional `child` is supported (rendered behind/over the image — painted
// after the image draw, matching the old behaviour where children render on
// top).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ImageSpec {
    pub resource_id: Option<Val<u64>>,
    pub width: Option<Val<f64>>,
    pub height: Option<Val<f64>>,
    pub fit: Option<Val<BoxFit>>,
    pub query_key: Option<Vec<String>>,
    pub child: Option<Rc<dyn Spec>>,
}

impl Spec for ImageSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(id, AnyElement::new(Image { spec: self.clone() }), boa);
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id);
        }
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// Image — the built element. Layout and paint read `Val<T>` props on demand.
// ---------------------------------------------------------------------------

pub struct Image {
    pub spec: ImageSpec,
}

impl Effect for Image {}

impl ElementTrace for Image {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(Val::Static(rid)) = &self.spec.resource_id {
            parts.push(format!("resource={rid}"));
        }
        if let Some(Val::Static(w)) = &self.spec.width {
            parts.push(format!("width={w}"));
        }
        if let Some(Val::Static(h)) = &self.spec.height {
            parts.push(format!("height={h}"));
        }
        if let Some(Val::Static(f)) = &self.spec.fit {
            parts.push(format!("fit={f:?}"));
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

/// Extract the optional child spec from a JS props object.
fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Spec>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_spec(&v)
}

impl ImageSpec {
    /// Build an `ImageSpec` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        ImageSpec {
            resource_id: prop_val::<u64>(props, "resourceId", ctx),
            width: prop_val::<f64>(props, "width", ctx),
            height: prop_val::<f64>(props, "height", ctx),
            fit: prop_val::<BoxFit>(props, "fit", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}
