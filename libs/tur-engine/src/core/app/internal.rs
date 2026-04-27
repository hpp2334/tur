use std::collections::{HashMap, HashSet};
use std::fmt;

use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::Constraints;

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::event::AppEvent;
use crate::core::focus::{FocusEventType, FocusManager};
use crate::core::fonts::FontManager;
use crate::core::gesture::{ComposedGestureEventKind, GestureEventComposer};
use crate::core::keyboard::{AppKeyEvent, KeyEventType};
use crate::core::render::Renderer;
use crate::elements::input::{InputEditResult, InputElement};

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
    focus_handlers: HashMap<(ElementNodeId, FocusEventType), JsObject>,
    pub(crate) input_nodes: HashSet<ElementNodeId>,
    pub(crate) text_input_callbacks: HashMap<ElementNodeId, JsObject>,
    pub(crate) text_input_focus_handlers: HashMap<(ElementNodeId, FocusEventType), JsObject>,
    pub(crate) text_input_cursor_handlers: HashMap<ElementNodeId, JsObject>,
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
            input_nodes: HashSet::new(),
            text_input_callbacks: HashMap::new(),
            text_input_focus_handlers: HashMap::new(),
            text_input_cursor_handlers: HashMap::new(),
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

        self.renderer.render(&self.element_tree);
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
        event_type: FocusEventType,
    ) -> Option<JsObject> {
        self.focus_handlers.get(&(id, event_type)).cloned()
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

    pub fn request_focus(&mut self, new_id: ElementNodeId) -> Option<ElementNodeId> {
        self.focus_manager.request_focus(new_id)
    }

    pub fn clear_focus(&mut self) -> Option<ElementNodeId> {
        self.focus_manager.clear_focus()
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
        focus_type: FocusEventType,
        handler: JsObject,
    ) {
        self.focus_handlers.insert((id, focus_type), handler);
    }

    pub fn remove_focus_handler(&mut self, id: ElementNodeId, focus_type: FocusEventType) {
        self.focus_handlers.remove(&(id, focus_type));
    }

    pub fn collect_key_event_data(
        &mut self,
        event: &AppKeyEvent,
        context: &mut Context,
    ) -> Option<(Vec<JsObject>, JsValue)> {
        let focused_id = self.focus_manager.focused()?;

        if self.input_nodes.contains(&focused_id) {
            self.handle_input_key(focused_id, event, context);
            return None;
        }

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
            return None;
        }

        let event_obj = crate::core::bridge::element_bridge::build_key_event_object(
            &event.key,
            &event.code,
            &event.modifiers,
            context,
        );

        Some((callbacks, event_obj))
    }

    fn handle_input_key(
        &mut self,
        node_id: ElementNodeId,
        event: &AppKeyEvent,
        context: &mut Context,
    ) {
        let node = self.element_tree.get_mut(node_id).unwrap();
        let element = node.element.as_mut().unwrap();
        let input_el = element.cast_mut::<InputElement>().unwrap();

        let result =
            input_el.handle_key_event(&event.key, event.modifiers.ctrl, event.modifiers.meta);

        let cursor_pos = input_el.cursor_position;
        let text = input_el.text().to_string();

        match result {
            InputEditResult::TextChanged(_) => {
                self.invoke_input_callback(node_id, &text, false, context);
                self.invoke_input_cursor_callback(node_id, cursor_pos, context);
                self.push_event(AppEvent::RequestDraw);
            }
            InputEditResult::EnterPressed(_) => {
                self.invoke_input_callback(node_id, &text, true, context);
                self.push_event(AppEvent::RequestDraw);
            }
            InputEditResult::CursorMoved => {
                self.invoke_input_cursor_callback(node_id, cursor_pos, context);
                self.push_event(AppEvent::RequestDraw);
            }
            InputEditResult::Handled => {
                self.push_event(AppEvent::RequestDraw);
            }
            InputEditResult::NotHandled => {}
        }
    }

    fn invoke_input_callback(
        &self,
        node_id: ElementNodeId,
        text: &str,
        enter: bool,
        context: &mut Context,
    ) {
        if let Some(cb) = self.text_input_callbacks.get(&node_id) {
            let text_val = boa_engine::JsValue::from(js_string!(text));
            let enter_val = boa_engine::JsValue::from(enter);
            let _ = cb.call(
                &boa_engine::JsValue::undefined(),
                &[text_val, enter_val],
                context,
            );
        }
    }

    fn invoke_input_cursor_callback(
        &self,
        node_id: ElementNodeId,
        pos: usize,
        context: &mut Context,
    ) {
        if let Some(cb) = self.text_input_cursor_handlers.get(&node_id) {
            let pos_val = boa_engine::JsValue::from(pos as f64);
            let _ = cb.call(&boa_engine::JsValue::undefined(), &[pos_val], context);
        }
    }

    fn set_input_focused(&mut self, node_id: ElementNodeId, focused: bool) {
        if let Some(node) = self.element_tree.get_mut(node_id) {
            if let Some(ref mut element) = node.element {
                if let Some(input_el) = element.cast_mut::<InputElement>() {
                    input_el.set_focused(focused);
                }
            }
        }
    }

    pub fn focus_input_if_hit(&mut self, path: &[ElementNodeId], context: &mut Context) -> bool {
        let input_id = path.iter().find(|id| self.input_nodes.contains(id));
        let Some(&id) = input_id else {
            return false;
        };

        let old_id = self.focus_manager.request_focus(id);
        if let Some(old) = old_id {
            self.set_input_focused(old, false);
            self.invoke_input_focus_handler(old, FocusEventType::Blur, context);
        }
        self.set_input_focused(id, true);
        self.invoke_input_focus_handler(id, FocusEventType::Focus, context);
        self.push_event(AppEvent::RequestDraw);
        true
    }

    fn invoke_input_focus_handler(
        &self,
        node_id: ElementNodeId,
        event_type: FocusEventType,
        context: &mut Context,
    ) {
        if let Some(cb) = self.text_input_focus_handlers.get(&(node_id, event_type)) {
            let _ = cb.call(&boa_engine::JsValue::undefined(), &[], context);
        }
    }

    pub fn find_focusable_in_path(&self, path: &[ElementNodeId]) -> Option<ElementNodeId> {
        for &id in path {
            if let Some(node) = self.element_tree.get(id) {
                if let Some(ref element) = node.element {
                    if element.type_name() == "tur_focusable" {
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
