use std::rc::Rc;

use boa_engine::object::builtins::JsArray;
use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use tur_shared::{CrossAxisAlignment, MainAxisSize, MainAxisAlignment};

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::reactive::{extract_atom, AtomId};
use crate::core::widget::{extract_component, val_from_js, Effect, PropValue, Component, Val, WidgetCx};
use crate::elements::flex::FlexElement;
use crate::elements::FlexComponent;

// ---------------------------------------------------------------------------
// EachComponent — render one child per item of a reactive array.
//
// `items` is an atom (source or derived) holding a JS array. `build` is a JS
// function `(item, index) => EdgyElement` invoked once per item to produce
// that item's subtree. Whenever the `items` atom changes, the mounted item
// subtrees are rebuilt. `EachElement` lays its children out as a vertical flex
// (Column); the layout is delegated to a `FlexElement` instance, so `mainAlignment`,
// `crossAlignment`, and `mainAxisSize` behave exactly like `Column`.
//
// Like `LazyListComponent`, this holds a `JsFunction`; the spec rides on
// `ComponentHandle`'s `unsafe_empty_trace`, so the build closure must be kept
// alive by the JS module scope for the lifetime of the app (it always is).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct EachComponent {
    pub items: AtomId,
    pub build: JsFunction,
    pub main_alignment: Option<Val<MainAxisAlignment>>,
    pub cross_alignment: Option<Val<CrossAxisAlignment>>,
    pub main_axis_size: Option<Val<MainAxisSize>>,
    pub query_key: Option<Vec<String>>,
}

/// Invoke the JS `build(item, index)` closure, returning the produced spec.
fn build_item_spec(
    builder: &JsFunction,
    item: &JsValue,
    index: u64,
    boa: &mut Context,
) -> Option<Rc<dyn Component>> {
    let result = builder
        .call(
            &JsValue::undefined(),
            &[item.clone(), JsValue::from(index as f64)],
            boa,
        )
        .ok()?;
    extract_component(&result)
}

impl EachComponent {
    /// Read the current `items` array from the store and build one child per
    /// entry under `parent`. Returns the built node ids in array order.
    fn build_items(
        &self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        parent: ElementNodeId,
    ) -> Vec<ElementNodeId> {
        let raw = cx.read_atom_raw(self.items, boa);
        let Some(arr) = raw.as_object().and_then(|o| JsArray::from_object(o.clone()).ok()) else {
            return Vec::new();
        };
        let len = arr.length(boa).unwrap_or(0);

        let mut out = Vec::with_capacity(len as usize);
        for i in 0..len as i64 {
            let Ok(item) = arr.at(i, boa) else {
                continue;
            };
            let Some(spec) = build_item_spec(&self.build, &item, i as u64, boa) else {
                continue;
            };
            out.push(spec.build(cx, boa, parent));
        }
        out
    }
}

impl Component for EachComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();

        // Build items BEFORE inserting this node so each item's self-link to
        // `id` is a no-op; we then explicitly link them after inserting to
        // yield a single edge per item (same pattern as `LazyListElement`).
        let mounted = self.build_items(cx, boa, id);
        let mounted_ids = mounted.clone();

        cx.insert_node(
            id,
            AnyElement::new(EachElement {
                flex: FlexElement {
                    component: FlexComponent {
                        // `EachElement` is a vertical list (Column-like).
                        direction: Some(tur_shared::Axis::Vertical),
                        main_alignment: self.main_alignment.clone(),
                        cross_alignment: self.cross_alignment.clone(),
                        main_axis_size: self.main_axis_size.clone(),
                        // Children live directly under the `EachElement` node, not
                        // inside this delegate — `children` is unused.
                        children: Vec::new(),
                        query_key: None,
                    },
                    child_data: Vec::new(),
                    constraints: None,
                    computed_size: None,
                },
                component: self.clone(),
                node_id: id,
                mounted,
            }),
            boa,
        );

        for item_id in &mounted_ids {
            cx.link_child(id, *item_id);
        }
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// EachElement — the built element.
// ---------------------------------------------------------------------------

pub struct EachElement {
    pub(crate) flex: FlexElement,
    pub component: EachComponent,
    pub(crate) node_id: ElementNodeId,
    pub(crate) mounted: Vec<ElementNodeId>,
}

impl Effect for EachElement {
    fn effect(
        &mut self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<AtomId>,
    ) {
        if !dirties.contains(&self.component.items) {
            return;
        }

        // Rebuild-all reconciliation: tear down every previously mounted item
        // and rebuild from the current array. Simple and correct; the item
        // subtrees are stateless widgets so rebuilding them is cheap.
        let node_id = self.node_id;
        for old in self.mounted.drain(..) {
            cx.destroy_subtree(old);
        }
        self.mounted = self.component.build_items(cx, boa, node_id);
        cx.mark_dirty(node_id);
    }
}

impl ElementTrace for EachElement {
    fn trace_label(&self) -> String {
        format!("items={}", self.mounted.len())
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        vec![("itemCount", TraceValue::Num(self.mounted.len() as f64))]
    }
}

// ---------------------------------------------------------------------------
// Factory — parse props into a spec.
// ---------------------------------------------------------------------------

fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

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
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn prop_items(props: &JsObject, key: &str, ctx: &mut Context) -> Option<AtomId> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_atom(&v)
}

fn prop_builder(props: &JsObject, key: &str, ctx: &mut Context) -> Option<JsFunction> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    v.as_object().and_then(JsFunction::from_object)
}

impl EachComponent {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        Some(EachComponent {
            items: prop_items(props, "items", ctx)?,
            build: prop_builder(props, "build", ctx)?,
            main_alignment: prop_val::<MainAxisAlignment>(props, "mainAlignment", ctx),
            cross_alignment: prop_val::<CrossAxisAlignment>(props, "crossAlignment", ctx),
            main_axis_size: prop_val::<MainAxisSize>(props, "mainAxisSize", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        })
    }
}
