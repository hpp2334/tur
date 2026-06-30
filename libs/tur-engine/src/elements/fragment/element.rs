use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::NodeId;
use crate::core::elements::ElementTrace;
use crate::core::view::{ViewCx, extract_view, Effect, View};

// ---------------------------------------------------------------------------
// FragmentView — a transparent multi-child container. Children are built
// directly under a single FragmentElement node (which renders nothing and sizes to
// the union of its children).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FragmentView {
    pub children: Vec<Rc<dyn View>>,
    pub query_key: Option<Vec<String>>,
}

impl View for FragmentView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        // FragmentElement is truly transparent — no node is created. Children are
        // built directly under the parent. This matches React FragmentElement
        // semantics and keeps the tree flat for tests that navigate
        // root.children directly.
        for child_spec in &self.children {
            child_spec.build(cx, boa, parent);
        }
        parent
    }
}

// ---------------------------------------------------------------------------
// FragmentElement element — only used if a FragmentElement node is ever created directly
// (normally FragmentElement is transparent and creates no node). Kept for potential
// future use.
// ---------------------------------------------------------------------------

pub struct FragmentElement {
    pub view: FragmentView,
}

impl crate::core::layout::ElementSubscribe for FragmentElement {}

impl Effect for FragmentElement {}

impl ElementTrace for FragmentElement {}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

/// Extract a `Vec<String>` prop (queryKey) — parsed eagerly.
fn prop_query_key(
    props: &JsObject,
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

/// Extract child specs from a JS array of ComponentHandle opaques.
fn prop_children(props: &JsObject, key: &str, ctx: &mut Context) -> Vec<Rc<dyn View>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
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

impl FragmentView {
    /// Build a `FragmentView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        FragmentView {
            children: prop_children(props, "children", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
