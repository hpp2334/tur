use std::cell::{Cell, RefCell};
use std::str::FromStr;

use boa_engine::boa_class;
use boa_engine::{Context, JsResult, JsValue};
use tracing;
use tur_widget::{PropValue, WidgetKind, WidgetNode, WidgetNodeId, WidgetTree};

use crate::BoaOpaque;

#[derive(Debug, Default, boa_engine::JsData, boa_engine::Trace, boa_engine::Finalize)]
pub struct TurAppContext {
    pub tree: BoaOpaque<RefCell<WidgetTree>>,
    pub next_id: BoaOpaque<Cell<u64>>,
}

#[boa_class(rename = "TurApp")]
#[allow(non_snake_case)]
impl TurAppContext {
    #[boa(constructor)]
    pub fn constructor() -> JsResult<Self> {
        Ok(Self {
            tree: BoaOpaque(RefCell::new(WidgetTree::new())),
            next_id: BoaOpaque(Cell::new(1)),
        })
    }

    pub fn createElement(&self, kind_str: String) -> JsResult<f64> {
        let kind = WidgetKind::from_str(&kind_str).unwrap_or_else(|_| {
            tracing::warn!("unknown widget type: {kind_str}, falling back to Container");
            WidgetKind::Container
        });

        let id = self.alloc_id();
        let node = WidgetNode::new(id, kind);
        self.tree.borrow_mut().insert(node);

        tracing::trace!("createElement({kind_str}) -> {}", id.as_u64());
        Ok(id.as_u64() as f64)
    }

    pub fn createRoot(&self) -> JsResult<f64> {
        let id = self.alloc_id();
        let node = WidgetNode::new(id, WidgetKind::Column);
        self.tree.borrow_mut().insert(node);

        tracing::trace!("createRoot() -> {}", id.as_u64());
        Ok(id.as_u64() as f64)
    }

    pub fn setAttribute(
        &self,
        id: f64,
        key: String,
        value: JsValue,
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let node_id = WidgetNodeId::new(id as u64);

        let prop_value = if let Some(s) = value.as_string() {
            PropValue::String(s.to_std_string_escaped())
        } else if let Some(n) = value.as_number() {
            PropValue::Number(n)
        } else if let Some(b) = value.as_boolean() {
            PropValue::Bool(b)
        } else if let Some(b) = value.as_bigint() {
            let n: i64 = b.to_string().parse().unwrap_or(0);
            PropValue::Number(n as f64)
        } else {
            PropValue::String(
                value
                    .to_string(context)
                    .map(|s| s.to_std_string_escaped())
                    .unwrap_or_default(),
            )
        };

        tracing::trace!("setAttribute({}, {key}, ...)", node_id.as_u64());

        if let Some(node) = self.tree.borrow_mut().get_mut(node_id) {
            node.set_prop(key, prop_value);
        }

        Ok(JsValue::undefined())
    }

    pub fn appendChild(&self, parent: f64, child: f64) -> JsResult<JsValue> {
        let parent_id = WidgetNodeId::new(parent as u64);
        let child_id = WidgetNodeId::new(child as u64);

        self.tree.borrow_mut().append_child(parent_id, child_id);

        tracing::trace!("appendChild({}, {})", parent_id.as_u64(), child_id.as_u64());
        Ok(JsValue::undefined())
    }

    pub fn removeChild(&self, parent: f64, child: f64) -> JsResult<JsValue> {
        let parent_id = WidgetNodeId::new(parent as u64);
        let child_id = WidgetNodeId::new(child as u64);

        self.tree.borrow_mut().remove_child(parent_id, child_id);

        tracing::trace!("removeChild({}, {})", parent_id.as_u64(), child_id.as_u64());
        Ok(JsValue::undefined())
    }

    pub fn insertBefore(&self, parent: f64, child: f64, reference: f64) -> JsResult<JsValue> {
        let parent_id = WidgetNodeId::new(parent as u64);
        let child_id = WidgetNodeId::new(child as u64);
        let ref_id = WidgetNodeId::new(reference as u64);

        self.tree
            .borrow_mut()
            .insert_before(parent_id, child_id, ref_id);

        tracing::trace!(
            "insertBefore({}, {}, {})",
            parent_id.as_u64(),
            child_id.as_u64(),
            ref_id.as_u64()
        );
        Ok(JsValue::undefined())
    }

    pub fn getParent(&self, id: f64) -> JsResult<JsValue> {
        let node_id = WidgetNodeId::new(id as u64);
        match self.tree.borrow().parent_of(node_id) {
            Some(parent_id) => Ok(JsValue::from(parent_id.as_u64() as f64)),
            None => Ok(JsValue::null()),
        }
    }

    pub fn getFirstChild(&self, id: f64) -> JsResult<JsValue> {
        let node_id = WidgetNodeId::new(id as u64);
        match self.tree.borrow().first_child_of(node_id) {
            Some(child_id) => Ok(JsValue::from(child_id.as_u64() as f64)),
            None => Ok(JsValue::null()),
        }
    }

    pub fn getNextSibling(&self, id: f64) -> JsResult<JsValue> {
        let node_id = WidgetNodeId::new(id as u64);
        match self.tree.borrow().next_sibling_of(node_id) {
            Some(sibling_id) => Ok(JsValue::from(sibling_id.as_u64() as f64)),
            None => Ok(JsValue::null()),
        }
    }
}

impl TurAppContext {
    fn alloc_id(&self) -> WidgetNodeId {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        WidgetNodeId::new(id)
    }
}
