use std::fmt;

use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::Constraints;

use crate::core::element::ElementNodeId;
use crate::core::elements::{
    ComposedGestureEvent, ElementOnGestureContext, ElementOnKeyboardContext, ElementTree, GestureResult, KeyboardResult,
};
use crate::core::event::AppEvent;
use crate::core::focus::FocusManager;
use crate::core::fonts::FontManager;
use crate::core::gesture::GestureEventComposer;
use crate::core::js_event::JsEventQueue;
use crate::core::keyboard::AppKeyEvent;
use crate::core::render::Renderer;

pub struct TurAppInternal {
    element_tree: ElementTree,
    renderer: Box<dyn Renderer>,
    font_manager: FontManager,
    text_layout_cx: ParleyLayoutContext<[u8; 4]>,
    size: (f64, f64),
    gesture_composer: GestureEventComposer,
    focus_manager: FocusManager,
    event_queue: Vec<AppEvent>,
    js_event_queue: JsEventQueue,
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
            gesture_composer: GestureEventComposer::new(),
            focus_manager: FocusManager::new(),
            event_queue: Vec::new(),
            js_event_queue: JsEventQueue::new(),
        }
    }

    pub fn element_tree(&self) -> &ElementTree {
        &self.element_tree
    }

    pub fn element_tree_mut(&mut self) -> &mut ElementTree {
        &mut self.element_tree
    }

    pub fn js_event_queue_mut(&mut self) -> &mut JsEventQueue {
        &mut self.js_event_queue
    }

    pub fn focus_manager_mut(&mut self) -> &mut FocusManager {
        &mut self.focus_manager
    }

    pub fn set_focus(&mut self, new_id: ElementNodeId) {
        self.focus_manager.set_focus(new_id, &mut self.js_event_queue);
    }

    pub fn clear_focus(&mut self) {
        self.focus_manager.clear_focus(&mut self.js_event_queue);
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

    pub fn hit_test(&self, position: tur_shared::Offset) -> Option<ElementNodeId> {
        self.element_tree.hit_test_path(position).first().copied()
    }

    pub fn hit_test_contains(&self, position: tur_shared::Offset, id: ElementNodeId) -> bool {
        self.element_tree.hit_test_path(position).contains(&id)
    }

    pub fn compose_pointer_down(&mut self, target: Option<ElementNodeId>) {
        self.gesture_composer.on_pointer_down(target);
    }

    pub fn compose_pointer_up(&mut self, click_eligible: bool) -> bool {
        self.gesture_composer.on_pointer_up(click_eligible).is_some()
    }

    pub fn gesture_pointer_down_target(&self) -> Option<ElementNodeId> {
        self.gesture_composer.pointer_down_target()
    }

    pub fn is_gesture_dragging(&self) -> bool {
        self.gesture_composer.is_tracking_drag()
    }

    pub fn focused_element(&self) -> Option<ElementNodeId> {
        self.focus_manager.focused()
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
            self.focus_manager.set_focus(id, &mut self.js_event_queue);
        }

        if matches!(result, GestureResult::NeedsDraw) {
            self.push_event(AppEvent::RequestDraw);
        }

        result
    }

    pub fn handle_key_event(&mut self, event: &AppKeyEvent) -> KeyboardResult {
        let focused_id = match self.focus_manager.focused() {
            Some(id) => id,
            None => return KeyboardResult::NotHandled,
        };

        let mut redraw = false;
        let mut cx = ElementOnKeyboardContext::new(
            &mut self.js_event_queue,
            focused_id,
            &mut redraw,
        );

        let result = {
            let node = self.element_tree.get_mut(focused_id).unwrap();
            let element = node.element.as_mut().unwrap();
            element.on_keyboard_event(&mut cx, event)
        };

        if redraw {
            self.push_event(AppEvent::RequestDraw);
        }

        if matches!(result, KeyboardResult::NeedsDraw) {
            self.push_event(AppEvent::RequestDraw);
        }

        result
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
