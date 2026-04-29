use std::collections::HashMap;
use std::fmt;

use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::Constraints;

use crate::core::element::ElementNodeId;
use crate::core::elements::{
    ComposedGestureEvent, ElementOnGestureContext, ElementTree, GestureChanges, GestureResult,
    KeyboardResult,
};
use crate::core::event::AppEvent;
use crate::core::focus::FocusManager;
use crate::core::fonts::FontManager;
use crate::core::gesture::{ComposedGestureEventKind, GestureEventComposer};
use crate::core::keyboard::{AppKeyEvent, KeyEventType};
use crate::core::render::Renderer;

pub struct TurAppInternal {
    element_tree: ElementTree,
    renderer: Box<dyn Renderer>,
    font_manager: FontManager,
    text_layout_cx: ParleyLayoutContext<[u8; 4]>,
    size: (f64, f64),
    event_handlers: HashMap<(ElementNodeId, ComposedGestureEventKind), JsObject>,
    gesture_composer: GestureEventComposer,
    focus_manager: FocusManager,
    key_handlers: HashMap<(ElementNodeId, KeyEventType), JsObject>,
    focus_handlers: HashMap<ElementNodeId, JsObject>,
    blur_handlers: HashMap<ElementNodeId, JsObject>,
    pub(crate) text_input_callbacks: HashMap<ElementNodeId, JsObject>,
    pub(crate) text_input_cursor_handlers: HashMap<ElementNodeId, JsObject>,
    pub(crate) text_input_selection_handlers: HashMap<ElementNodeId, JsObject>,
    event_queue: Vec<AppEvent>,
}

impl fmt::Debug for TurAppInternal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurAppInternal")
            .field("element_tree", &self.element_tree)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl TurAppInternal {
    pub fn new(
        renderer: Box<dyn Renderer>,
        font_loader: Box<dyn crate::core::fonts::FontLoader>,
    ) -> Self {
        let font_manager = FontManager::new(font_loader);
        Self {
            element_tree: ElementTree::new(),
            renderer,
            font_manager,
            text_layout_cx: ParleyLayoutContext::new(),
            size: (400.0, 600.0),
            event_handlers: HashMap::new(),
            gesture_composer: GestureEventComposer::new(),
            focus_manager: FocusManager::new(),
            key_handlers: HashMap::new(),
            focus_handlers: HashMap::new(),
            blur_handlers: HashMap::new(),
            text_input_callbacks: HashMap::new(),
            text_input_cursor_handlers: HashMap::new(),
            text_input_selection_handlers: HashMap::new(),
            event_queue: Vec::new(),
        }
    }

    pub fn element_tree(&self) -> &ElementTree {
        &self.element_tree
    }

    pub fn element_tree_mut(&mut self) -> &mut ElementTree {
        &mut self.element_tree
    }

    pub fn set_size(&mut self, width: f64, height: f64) {
        self.size = (width, height);
    }

    pub fn render(&mut self) {
        let (width, height) = self.size;
        let constraints = Constraints {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        };

        let layout_size = self.element_tree.compute_layout(
            &constraints,
            &mut self.font_manager,
            &mut self.text_layout_cx,
        );
        tracing::debug!("layout: {:?}", layout_size);

        let focused_node_id = self.focus_manager.focused();
        self.renderer.render(&self.element_tree, focused_node_id);
    }

    pub fn push_event(&mut self, event: AppEvent) {
        self.event_queue.push(event);
    }

    pub fn drain_events(&mut self) -> Vec<AppEvent> {
        std::mem::take(&mut self.event_queue)
    }

    pub fn has_event_handler(&self, id: ElementNodeId, kind: ComposedGestureEventKind) -> bool {
        self.event_handlers.contains_key(&(id, kind))
    }

    pub fn set_event_handler(
        &mut self,
        id: ElementNodeId,
        kind: ComposedGestureEventKind,
        handler: JsObject,
    ) {
        self.event_handlers.insert((id, kind), handler);
    }

    pub fn remove_event_handler(&mut self, id: ElementNodeId, kind: ComposedGestureEventKind) {
        self.event_handlers.remove(&(id, kind));
    }

    pub fn collect_event_handlers(
        &self,
        kind: ComposedGestureEventKind,
        position: tur_shared::Offset,
    ) -> Vec<JsObject> {
        let path = self.element_tree.hit_test_path(position);
        path.iter()
            .filter_map(|id| self.event_handlers.get(&(*id, kind)).cloned())
            .collect()
    }

    pub fn collect_focus_handler(
        &self,
        id: ElementNodeId,
    ) -> Option<JsObject> {
        self.focus_handlers.get(&id).cloned()
    }

    pub fn collect_blur_handler(
        &self,
        id: ElementNodeId,
    ) -> Option<JsObject> {
        self.blur_handlers.get(&id).cloned()
    }

    pub fn hit_test(&self, position: tur_shared::Offset) -> Option<ElementNodeId> {
        self.element_tree.hit_test_path(position).first().copied()
    }

    pub fn hit_test_contains(&self, position: tur_shared::Offset, id: ElementNodeId) -> bool {
        self.element_tree.hit_test_path(position).contains(&id)
    }

    pub fn compose_pointer_down(&mut self, target: Option<ElementNodeId>) {
        self.gesture_composer.on_pointer_down(target);
    }

    pub fn compose_pointer_up(&mut self, click_eligible: bool) -> Option<ComposedGestureEventKind> {
        self.gesture_composer.on_pointer_up(click_eligible)
    }

    pub fn gesture_pointer_down_target(&self) -> Option<ElementNodeId> {
        self.gesture_composer.pointer_down_target()
    }

    pub fn is_gesture_dragging(&self) -> bool {
        self.gesture_composer.is_tracking_drag()
    }

    pub fn request_focus(&mut self, new_id: ElementNodeId) -> Option<ElementNodeId> {
        self.focus_manager.request_focus(new_id)
    }

    pub fn clear_focus(&mut self) -> Option<ElementNodeId> {
        self.focus_manager.clear_focus()
    }

    pub fn focused_element(&self) -> Option<ElementNodeId> {
        self.focus_manager.focused()
    }

    pub fn set_key_handler(
        &mut self,
        id: ElementNodeId,
        key_type: KeyEventType,
        handler: JsObject,
    ) {
        self.key_handlers.insert((id, key_type), handler);
    }

    pub fn remove_key_handler(&mut self, id: ElementNodeId, key_type: KeyEventType) {
        self.key_handlers.remove(&(id, key_type));
    }

    pub fn set_focus_handler(
        &mut self,
        id: ElementNodeId,
        handler: JsObject,
    ) {
        self.focus_handlers.insert(id, handler);
    }

    pub fn remove_focus_handler(&mut self, id: ElementNodeId) {
        self.focus_handlers.remove(&id);
    }

    pub fn set_blur_handler(
        &mut self,
        id: ElementNodeId,
        handler: JsObject,
    ) {
        self.blur_handlers.insert(id, handler);
    }

    pub fn remove_blur_handler(&mut self, id: ElementNodeId) {
        self.blur_handlers.remove(&id);
    }

    pub fn local_position(&self, node_id: ElementNodeId, global: tur_shared::Offset) -> tur_shared::Offset {
        let mut abs_x = 0.0f64;
        let mut abs_y = 0.0f64;
        let mut current = Some(node_id);
        while let Some(cid) = current {
            if let Some(n) = self.element_tree.get(cid) {
                abs_x += n.computed_layout.offset.x;
                abs_y += n.computed_layout.offset.y;
                current = n.parent;
            } else {
                break;
            }
        }
        tur_shared::Offset::new(global.x - abs_x, global.y - abs_y)
    }

    pub fn handle_gesture_event(
        &mut self,
        node_id: ElementNodeId,
        event: &ComposedGestureEvent,
        context: &mut Context,
    ) -> GestureResult {
        let mut redraw = false;
        let mut focus_req: Option<ElementNodeId> = None;
        let mut cx = ElementOnGestureContext::new(&mut redraw, &mut focus_req);

        let result = {
            let node = match self.element_tree.get_mut(node_id) {
                Some(n) => n,
                None => return GestureResult::NotHandled,
            };
            let element = match node.element.as_mut() {
                Some(e) => e,
                None => return GestureResult::NotHandled,
            };
            element.on_gesture_event(event, &mut cx)
        };

        if redraw {
            self.push_event(AppEvent::RequestDraw);
        }
        if let Some(id) = focus_req {
            self.focus_manager.request_focus(id);
        }

        if matches!(result, GestureResult::NeedsDraw) {
            let changes = {
                let node = self.element_tree.get_mut(node_id).unwrap();
                let element = node.element.as_mut().unwrap();
                element.drain_changes()
            };
            self.fire_element_callbacks(node_id, &changes, context);
            self.push_event(AppEvent::RequestDraw);
        }

        result
    }

    pub fn handle_key_event(
        &mut self,
        event: &AppKeyEvent,
        context: &mut Context,
    ) {
        let focused_id = match self.focus_manager.focused() {
            Some(id) => id,
            None => return,
        };

        let result = {
            let node = self.element_tree.get_mut(focused_id).unwrap();
            let element = node.element.as_mut().unwrap();
            element.on_keyboard_event(event)
        };

        match result {
            KeyboardResult::NotHandled => {
                self.dispatch_js_key_handlers(focused_id, event, context);
            }
            KeyboardResult::Handled => {}
            KeyboardResult::NeedsDraw => {
                let changes = {
                    let node = self.element_tree.get_mut(focused_id).unwrap();
                    let element = node.element.as_mut().unwrap();
                    element.drain_changes()
                };
                self.fire_element_callbacks(focused_id, &changes, context);
                self.push_event(AppEvent::RequestDraw);
            }
        }
    }

    fn fire_element_callbacks(
        &self,
        node_id: ElementNodeId,
        changes: &GestureChanges,
        context: &mut Context,
    ) {
        if changes.text_changed {
            self.fire_on_input(node_id, changes, context);
        }

        if changes.cursor_changed {
            self.fire_on_cursor_change(node_id, changes, context);
        }

        if changes.selection_changed {
            self.fire_on_selection_change(node_id, changes, context);
        }
    }

    fn fire_on_input(
        &self,
        node_id: ElementNodeId,
        changes: &GestureChanges,
        context: &mut Context,
    ) {
        if let Some(cb) = self.text_input_callbacks.get(&node_id) {
            let text = self
                .element_tree
                .get(node_id)
                .and_then(|n| n.element.as_ref())
                .and_then(|e| e.cast::<crate::elements::InputElement>())
                .map(|i| i.text().to_string())
                .unwrap_or_default();
            let text_val = JsValue::from(js_string!(text.as_str()));
            let enter_val = JsValue::from(changes.enter);
            let _ = cb.call(&JsValue::undefined(), &[text_val, enter_val], context);
        }
    }

    fn fire_on_cursor_change(
        &self,
        node_id: ElementNodeId,
        _changes: &GestureChanges,
        context: &mut Context,
    ) {
        if let Some(cb) = self.text_input_cursor_handlers.get(&node_id) {
            let pos = self
                .element_tree
                .get(node_id)
                .and_then(|n| n.element.as_ref())
                .and_then(|e| e.cast::<crate::elements::InputElement>())
                .map(|i| i.cursor_position())
                .unwrap_or(0);
            let pos_val = JsValue::from(pos as f64);
            let _ = cb.call(&JsValue::undefined(), &[pos_val], context);
        }
    }

    fn fire_on_selection_change(
        &self,
        node_id: ElementNodeId,
        _changes: &GestureChanges,
        context: &mut Context,
    ) {
        if let Some(cb) = self.text_input_selection_handlers.get(&node_id) {
            let (anchor, end) = self
                .element_tree
                .get(node_id)
                .and_then(|n| n.element.as_ref())
                .and_then(|e| e.cast::<crate::elements::InputElement>())
                .map(|i| (i.selection_anchor(), i.selection_end()))
                .unwrap_or((0, 0));
            let start_val = JsValue::from(anchor as f64);
            let end_val = JsValue::from(end as f64);
            let _ = cb.call(&JsValue::undefined(), &[start_val, end_val], context);
        }
    }

    fn dispatch_js_key_handlers(
        &mut self,
        focused_id: ElementNodeId,
        event: &AppKeyEvent,
        context: &mut Context,
    ) {
        let path = {
            let mut path = Vec::new();
            let mut current = Some(focused_id);
            while let Some(id) = current {
                path.push(id);
                current = self.element_tree.parent_of(id);
            }
            path
        };

        let callbacks: Vec<JsObject> = path
            .iter()
            .filter_map(|id| self.key_handlers.get(&(*id, event.event_type)).cloned())
            .collect();

        if callbacks.is_empty() {
            return;
        }

        let event_obj = crate::core::bridge::element_bridge::build_key_event_object(
            &event.key,
            &event.code,
            &event.modifiers,
            context,
        );

        for callback in callbacks {
            let result: Result<JsValue, _> = callback.call(
                &JsValue::undefined(),
                std::slice::from_ref(&event_obj),
                context,
            );
            match result {
                Ok(r) if r.to_boolean() => break,
                _ => continue,
            }
        }
    }

    pub fn find_focusable_in_path(&self, path: &[ElementNodeId]) -> Option<ElementNodeId> {
        for &id in path {
            if let Some(node) = self.element_tree.get(id) {
                if let Some(ref element) = node.element {
                    if element.has_focus() {
                        return Some(id);
                    }
                }
            }
        }
        None
    }

    pub fn set_node_query_key(&mut self, id: ElementNodeId, keys: Option<Vec<String>>) {
        if let Some(node) = self.element_tree.get_mut(id) {
            node.query_key = keys;
        }
    }

    pub fn resize_renderer(&mut self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.renderer.resize(logical_width, logical_height, dpr);
    }

    pub fn present_renderer(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.renderer.present()
    }

    pub fn debug_layout(&self) -> String {
        self.element_tree.debug_layout()
    }
}
