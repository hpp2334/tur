//! View roots — the engine's per-instance mount slots.
//!
//! **One element tree per view root.** Each [`RootSlot`] owns its own
//! [`NodeTree`], its own [`Screen`] (logical size + dpr + `viewportSize$`
//! atom), its host-written `active$` mirror, and the mount intent (the view
//! spec set via `setViewRoot`). The [`ViewRootRegistry`] keeps slots in
//! registration order and provides id/name lookup plus tree routing (find
//! the tree that owns a node id — node ids carry their root, so routing is
//! O(1)).
//!
//! ## Lifecycle (deferred surfaces)
//!
//! Roots are declared at build time
//! (`TurAppBuilder::view_root(name, viewport, dpr)`) and start **pending**:
//! the registry slot, `Screen`, and atoms exist, but there is no render
//! target and no built tree (`active$ = false`). The host attaches the
//! surface — which may appear arbitrarily late (a page the user hasn't
//! visited yet) — via `TurApp::setup_root(name, surface, viewport, dpr)`,
//! and detaches via `TurApp::tear_down_root(name)`:
//!
//! | Transition | Behavior |
//! |---|---|
//! | `setup_root` | Creates the root's render target from the surface on main (fail-fast on a mismatched pairing; replays retained image uploads), then rebuilds the tree from the mount intent (mount hooks fire). `active$` → `true`. |
//! | `tear_down` | Releases the root's render target (frees the GPU/GL resources) and destroys the built subtree (unmount hooks fire, subscriptions cleaned) — same machinery as a `Switch` branch swap. The mount **intent** is retained. |
//! | `setViewRoot` while torn-down / pending | Records the intent only; the build is deferred until the next `setup_root`. |
//! | `resetViewRoot` | Destroys the built tree AND clears the intent. |
//! | `resize_root` (any state) | Updates the worker `Screen` / `viewportSize$`; the main-side target resize no-ops while detached. |
//!
//! The reactive store is shared instance-wide, so JS-minted atoms and each
//! root's `viewportSize$` are visible across roots; only element-local state
//! (controllers, editable text) resets on teardown/rebuild — identical to
//! `Switch`/`Condition` remount semantics.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::{Context, JsValue};
use boa_gc::{Finalize, Trace};
use vello_common::kurbo::Rect;

use crate::core::edgy::reactive::Source;
use crate::core::edgy::reactive::Store;
use crate::core::element::{ElementNodeId, NodeId, ViewRootId};
use crate::core::elements::NodeTree;
use crate::core::js_runtime::js_value::IntoJs;
use crate::core::screen::Screen;
use crate::core::view::View;

/// One view root: name + own tree + own screen + lifecycle + mount intent.
pub struct RootSlot {
    pub name: String,
    pub id: ViewRootId,
    /// This root's element tree — exactly one tree per view root. Node ids
    /// carry this root's `ViewRootId` (per-tree counters), so they never
    /// collide with other roots' trees.
    pub tree: NodeTree,
    /// This root's screen state: logical size, dpr, and the per-root
    /// `viewportSize$` source atom.
    pub screen: Screen,
    /// Host-written lifecycle mirror (`true` = setup). Read-only to JS;
    /// workers update it on `WorkerMsg::SetupRoot` / `TearDownRoot`.
    pub(crate) active_source: Source<JsValue>,
    /// The `active$` atom handle (minted once; cloned into every JS handle
    /// object so `get(root.active$)` sees one identity).
    pub(crate) active_js: JsValue,
    /// The `viewportSize$` atom handle (minted once; cloned into JS handle
    /// objects).
    pub(crate) viewport_size_js: JsValue,
    /// `true` while the root is set up (a surface is attached + the tree
    /// may be built). Roots start PENDING at build (`false`); the host
    /// flips them via `setup_root` / `tear_down_root`.
    pub(crate) setup: bool,
    /// Mount intent — the view spec set via `setViewRoot`. Survives
    /// teardown so `setup_root` can rebuild.
    pub(crate) mounted_handle: Option<Rc<dyn View>>,
    /// The built `RootElement` node id — present only while setup AND
    /// mounted.
    pub(crate) built_root: Option<ElementNodeId>,
}

impl RootSlot {
    /// The paint-pass viewport clip rect for this root (its logical size).
    pub fn viewport_rect(&self) -> Rect {
        let (w, h) = self.screen.logical_size;
        Rect::new(0.0, 0.0, w, h)
    }
}

/// The per-instance view-root registry. Shared (as `Rc<RefCell<…>>`) between
/// `TurAppContext` (flush driver) and `TurInstanceContext` (bridge fns).
pub struct ViewRootRegistry {
    slots: Vec<RootSlot>,
    by_name: HashMap<String, ViewRootId>,
    next_id: u32,
}

impl Default for ViewRootRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewRootRegistry {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            by_name: HashMap::new(),
            next_id: 0,
        }
    }

    /// Register one view root (build-time only). Mints the per-root
    /// `viewportSize$` + `active$` atoms from the shared store. Errors on a
    /// duplicate name.
    pub fn register(
        &mut self,
        name: &str,
        store: Store,
        viewport: (f64, f64),
        dpr: f64,
        boa: &mut Context,
    ) -> Result<ViewRootId, crate::error::TurError> {
        if self.by_name.contains_key(name) {
            return Err(crate::error::TurError::Other(format!(
                "duplicate view root name `{name}` — each `.view_root(...)` name must be unique"
            )));
        }
        let id = ViewRootId::new(self.next_id);
        self.next_id += 1;

        let mut screen = Screen::new(store.clone());
        screen.logical_size = viewport;
        screen.dpr = dpr;
        let init = Screen::size_js(viewport.0, viewport.1, boa);
        let viewport_source: Source<JsValue> = store.bridge().source(init);
        screen.set_source(viewport_source);
        let viewport_size_js = viewport_source.into_js(boa);
        // Roots start PENDING (`active$ = false`); `setup_root_impl` flips
        // the mirror when the host attaches a surface.
        let active_source: Source<JsValue> = store.bridge().source(JsValue::new(false));
        let active_js = active_source.into_js(boa);

        let tree = NodeTree::new_for_root(id, store);
        self.slots.push(RootSlot {
            name: name.to_string(),
            id,
            tree,
            screen,
            active_source,
            active_js,
            viewport_size_js,
            setup: false,
            mounted_handle: None,
            built_root: None,
        });
        self.by_name.insert(name.to_string(), id);
        Ok(id)
    }

    pub fn slots(&self) -> &[RootSlot] {
        &self.slots
    }

    pub fn get(&self, id: ViewRootId) -> Option<&RootSlot> {
        self.slots.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: ViewRootId) -> Option<&mut RootSlot> {
        self.slots.iter_mut().find(|s| s.id == id)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&RootSlot> {
        self.by_name.get(name).and_then(|id| self.get(*id))
    }

    pub fn id_of(&self, name: &str) -> Option<ViewRootId> {
        self.by_name.get(name).copied()
    }

    /// Root names in registration order (the `viewRoots()` bridge fn).
    pub fn names(&self) -> Vec<String> {
        self.slots.iter().map(|s| s.name.clone()).collect()
    }

    /// The tree of one root.
    pub fn tree_of_root(&self, id: ViewRootId) -> Option<NodeTree> {
        self.get(id).map(|s| s.tree.clone())
    }

    /// The tree that owns `node`. Node ids carry their owning root, so this
    /// is an O(1) root lookup plus a membership check — no tree scan. Returns
    /// the owning root id + a tree handle (None if the root doesn't exist or
    /// the node has been removed from its tree).
    pub fn tree_containing(&self, node: NodeId) -> Option<(ViewRootId, NodeTree)> {
        let slot = self.get(node.root())?;
        slot.tree
            .contains_node(node)
            .then(|| (slot.id, slot.tree.clone()))
    }

    /// Every tree, in registration order.
    pub fn trees(&self) -> Vec<NodeTree> {
        self.slots.iter().map(|s| s.tree.clone()).collect()
    }

    /// Every **setup** root's (id, tree) — what layout/paint iterate.
    pub fn setup_roots(&self) -> Vec<(ViewRootId, NodeTree)> {
        self.slots
            .iter()
            .filter(|s| s.setup)
            .map(|s| (s.id, s.tree.clone()))
            .collect()
    }

    /// Update the root's `active$` mirror.
    pub(crate) fn set_active(&mut self, id: ViewRootId, active: bool, boa: &mut Context) {
        let _ = boa;
        if let Some(slot) = self.get_mut(id) {
            let value = JsValue::new(active);
            let src = slot.active_source;
            let store = slot.screen.store.clone();
            store.bridge().set_source(src, value);
        }
    }
}

/// Shared registry handle type used across `TurAppContext` /
/// `TurInstanceContext` / `SubsystemFlushContext`.
pub type SharedViewRoots = Rc<RefCell<ViewRootRegistry>>;

/// Build-time declaration of one view root — what
/// `TurAppBuilder::view_root` captures and `spawn_instance` splits between
/// main (the retained renderer factory attaches surfaces later via
/// `MainBackend::attach_surface`) and worker (name/viewport → registry
/// slot). The root starts PENDING: no surface, no render target, no built
/// tree.
pub struct ViewRootSpec {
    pub name: String,
    pub viewport: (f64, f64),
    pub dpr: f64,
}

/// Boa-opaque wrapper so JS can hold a view-root handle (`viewRoot("main")`).
///
/// `unsafe_empty_trace` note: the struct holds `JsValue` atom handles, but in
/// this boa fork a `JsObject` clone held in Rust is a strong GC root via
/// refcounting, so the conservative empty trace cannot collect them
/// prematurely (worst case: a leak) — same trade-off as `ViewHandle`.
#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct ViewRootHandle {
    pub id: ViewRootId,
    /// The root's name (diagnostics; the JS handle object also carries it
    /// as a plain `name` property).
    #[allow(dead_code)]
    pub(crate) name: String,
}

impl ViewRootHandle {
    pub fn new(id: ViewRootId, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
        }
    }
}
