use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::Constraints;

use crate::core::bridge::element_bridge::TurNodeHandle;
use crate::core::bridge::BoaOpaque;
use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::event::AppEvent;
use crate::core::focus::{FocusEventType, FocusManager};
use crate::core::fonts::{FontLoader, FontManager};
use crate::core::gesture::{ComposedGestureEventKind, GestureEventComposer};
use crate::core::keyboard::{AppKeyEvent, KeyEventType};
use crate::core::render::Renderer;

pub struct TurAppInternal {
    pub(crate) element_tree: Rc<RefCell<ElementTree>>,
    pub(crate) renderer: RefCell<Box<dyn Renderer>>,
    font_manager: RefCell<FontManager>,
    text_layout_cx: RefCell<ParleyLayoutContext<[u8; 4]>>,
    size: Cell<(f64, f64)>,
    next_id: Cell<u64>,
    pub(crate) handles: RefCell<HashMap<ElementNodeId, BoaOpaque<TurNodeHandle>>>,
    pub(crate) event_handlers:
        RefCell<HashMap<(ElementNodeId, ComposedGestureEventKind), JsObject>>,
    gesture_composer: RefCell<GestureEventComposer>,
    focus_manager: RefCell<FocusManager>,
    pub(crate) key_handlers: RefCell<HashMap<(ElementNodeId, KeyEventType), JsObject>>,
    pub(crate) focus_handlers: RefCell<HashMap<(ElementNodeId, FocusEventType), JsObject>>,
    event_queue: RefCell<Vec<AppEvent>>,
}

impl fmt::Debug for TurAppInternal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurAppInternal")
            .field("element_tree", &self.element_tree)
            .field("size", &self.size)
            .field("next_id", &self.next_id)
            .field("handles", &self.handles)
            .finish_non_exhaustive()
    }
}

impl TurAppInternal {
    pub fn new(renderer: Box<dyn Renderer>, font_loader: Box<dyn FontLoader>) -> Self {
        let font_manager = FontManager::new(font_loader);
        Self {
            element_tree: Rc::new(RefCell::new(ElementTree::new())),
            renderer: RefCell::new(renderer),
            font_manager: RefCell::new(font_manager),
            text_layout_cx: RefCell::new(ParleyLayoutContext::new()),
            size: Cell::new((400.0, 600.0)),
            next_id: Cell::new(1),
            handles: RefCell::new(HashMap::new()),
            event_handlers: RefCell::new(HashMap::new()),
            gesture_composer: RefCell::new(GestureEventComposer::new()),
            focus_manager: RefCell::new(FocusManager::new()),
            key_handlers: RefCell::new(HashMap::new()),
            focus_handlers: RefCell::new(HashMap::new()),
            event_queue: RefCell::new(Vec::new()),
        }
    }

    pub fn element_tree(&self) -> &RefCell<ElementTree> {
        &self.element_tree
    }

    pub fn element_tree_rc(&self) -> Rc<RefCell<ElementTree>> {
        Rc::clone(&self.element_tree)
    }

    pub fn renderer(&self) -> &RefCell<Box<dyn Renderer>> {
        &self.renderer
    }

    pub fn set_size(&self, width: f64, height: f64) {
        self.size.set((width, height));
    }

    pub fn render(&self) {
        let (width, height) = self.size.get();
        let constraints = Constraints {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        };

        {
            let mut tree = self.element_tree.borrow_mut();
            let mut font_manager = self.font_manager.borrow_mut();
            let mut text_layout_cx = self.text_layout_cx.borrow_mut();
            let layout_size =
                tree.compute_layout(&constraints, &mut font_manager, &mut text_layout_cx);
            tracing::debug!("layout: {:?}", layout_size);
        }

        let mut renderer = self.renderer.borrow_mut();
        let tree = self.element_tree.borrow();
        renderer.render(&tree);
    }

    pub fn push_event(&self, event: AppEvent) {
        self.event_queue.borrow_mut().push(event);
    }

    pub fn drain_events(&self) -> Vec<AppEvent> {
        std::mem::take(&mut self.event_queue.borrow_mut())
    }

    pub fn alloc_id(&self) -> ElementNodeId {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        ElementNodeId::new(id)
    }

    pub fn get_or_create_handle(
        &self,
        id: ElementNodeId,
        context: &mut Context,
    ) -> BoaOpaque<TurNodeHandle> {
        let handles = self.handles.borrow();
        if let Some(opaque) = handles.get(&id) {
            let cloned: BoaOpaque<TurNodeHandle> = opaque.clone();
            return cloned;
        }
        drop(handles);
        let opaque = BoaOpaque::new(TurNodeHandle { id }, context);
        self.handles.borrow_mut().insert(id, opaque.clone());
        opaque
    }

    pub fn has_event_handler(&self, id: ElementNodeId, kind: ComposedGestureEventKind) -> bool {
        self.event_handlers.borrow().contains_key(&(id, kind))
    }

    pub fn invoke_handlers_for(
        &self,
        kind: ComposedGestureEventKind,
        position: tur_shared::Offset,
        context: &mut Context,
    ) {
        let path = {
            let tree = self.element_tree.borrow();
            tree.hit_test_path(position)
        };

        let callbacks: Vec<JsObject> = {
            let handlers = self.event_handlers.borrow();
            path.iter()
                .filter_map(|id| handlers.get(&(*id, kind)).cloned())
                .collect()
        };

        for callback in callbacks {
            let _ = callback.call(&boa_engine::JsValue::undefined(), &[], context);
        }
    }

    pub fn hit_test(&self, position: tur_shared::Offset) -> Option<ElementNodeId> {
        let tree = self.element_tree.borrow();
        tree.hit_test_path(position).first().copied()
    }

    pub fn hit_test_contains(&self, position: tur_shared::Offset, id: ElementNodeId) -> bool {
        let tree = self.element_tree.borrow();
        tree.hit_test_path(position).contains(&id)
    }

    pub fn compose_pointer_down(&self, target: Option<ElementNodeId>) {
        self.gesture_composer.borrow_mut().on_pointer_down(target);
    }

    pub fn compose_pointer_up(&self, click_eligible: bool) -> Option<ComposedGestureEventKind> {
        self.gesture_composer
            .borrow_mut()
            .on_pointer_up(click_eligible)
    }

    pub fn gesture_pointer_down_target(&self) -> Option<ElementNodeId> {
        self.gesture_composer.borrow().pointer_down_target()
    }

    pub fn request_focus(&self, new_id: ElementNodeId) -> Option<ElementNodeId> {
        self.focus_manager.borrow_mut().request_focus(new_id)
    }

    pub fn clear_focus(&self) -> Option<ElementNodeId> {
        self.focus_manager.borrow_mut().clear_focus()
    }

    pub fn invoke_focus_handlers(
        &self,
        id: ElementNodeId,
        event_type: FocusEventType,
        context: &mut Context,
    ) {
        let handlers = self.focus_handlers.borrow();
        if let Some(callback) = handlers.get(&(id, event_type)) {
            let _ = callback.call(&boa_engine::JsValue::undefined(), &[], context);
        }
    }

    pub fn dispatch_key_event(&self, event: &AppKeyEvent, context: &mut Context) {
        let focused_id = match self.focus_manager.borrow().focused() {
            Some(id) => id,
            None => return,
        };

        let path = {
            let tree = self.element_tree.borrow();
            let mut path = Vec::new();
            let mut current = Some(focused_id);
            while let Some(id) = current {
                path.push(id);
                current = tree.parent_of(id);
            }
            path
        };

        let callbacks: Vec<JsObject> = {
            let handlers = self.key_handlers.borrow();
            path.iter()
                .filter_map(|id| handlers.get(&(*id, event.event_type)).cloned())
                .collect()
        };

        let event_obj = crate::core::bridge::element_bridge::build_key_event_object(
            &event.key,
            &event.code,
            &event.modifiers,
            context,
        );

        for callback in callbacks {
            match callback.call(
                &boa_engine::JsValue::undefined(),
                std::slice::from_ref(&event_obj),
                context,
            ) {
                Ok(result) if result.to_boolean() => break,
                _ => continue,
            }
        }
    }

    pub fn find_focusable_in_path(&self, path: &[ElementNodeId]) -> Option<ElementNodeId> {
        let tree = self.element_tree.borrow();
        for &id in path {
            if let Some(node) = tree.get(id) {
                if let Some(ref element) = node.element {
                    if element.type_name() == "tur_focusable" {
                        return Some(id);
                    }
                }
            }
        }
        None
    }
}
