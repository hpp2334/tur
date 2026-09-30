use std::cell::OnceCell;
use std::fmt;

use crate::core::layout::{ComputedLayout, Constraints};
use boa_engine::Context;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::AnyElement;
use crate::core::js_runtime::{BoaOpaque, TurNodeHandle};

pub struct ElementObject {
    pub id: ElementNodeId,
    pub element: Option<AnyElement>,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub computed_layout: ComputedLayout,
    pub(crate) query_key: Option<Vec<String>>,
    /// The JS-visible node handle — materialized lazily, on first access
    /// from the JS side. A realm-free build (a rut-only instance) never
    /// allocates the JsObject, keeping element construction realm-free.
    handle: OnceCell<BoaOpaque<TurNodeHandle>>,
    pub(crate) dirty_layout: bool,
    pub(crate) last_constraints: Option<Constraints>,
}

impl fmt::Debug for ElementObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementObject")
            .field("id", &self.id)
            .field("kind", &self.element.as_ref().map(|e| e.kind()))
            .field("children", &self.children)
            .field("parent", &self.parent)
            .finish()
    }
}

impl ElementObject {
    /// Realm-free construction — the JS-visible handle (the `BoaOpaque`
    /// wrapping a `TurNodeHandle`) materializes lazily via [`Self::handle`],
    /// on the first access that carries a realm.
    pub fn new(id: ElementNodeId, element: AnyElement) -> Self {
        ElementObject {
            handle: OnceCell::new(),
            id,
            element: Some(element),
            children: Vec::new(),
            parent: None,
            computed_layout: ComputedLayout::ZERO,
            query_key: None,
            dirty_layout: true,
            last_constraints: None,
        }
    }

    /// The element's JS-visible node handle — created on first call (needs
    /// the realm), reused after. No in-tree caller today (controllers bind
    /// by node id at build time); this is the lazy materialization point
    /// for future JS-facing handle plumbing.
    #[allow(dead_code)]
    pub(crate) fn handle(&mut self, context: &mut Context) -> &BoaOpaque<TurNodeHandle> {
        self.handle
            .get_or_init(|| BoaOpaque::new(TurNodeHandle { id: self.id }, context))
    }
}
