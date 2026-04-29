pub mod core;
pub mod elements;
pub mod renderer;

pub mod error;

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::Context;
use boa_engine::Source;
use error::TurError;

use core::app::TurAppInternal;
use core::bridge::init_bridge;
use core::element::ElementNodeId;
use core::elements::AnyElement;
#[cfg(feature = "trace")]
use core::elements::ElementTree;
use core::elements::ComposedGestureEvent;
use core::event::{AppEvent, AppGestureEvent};

pub struct TurApp {
    boa_context: Context,
    app_context: Rc<RefCell<TurAppInternal>>,
}

impl TurApp {
    pub fn new(
        renderer: Box<dyn core::render::Renderer>,
        font_loader: Box<dyn core::fonts::FontLoader>,
    ) -> Result<Self, TurError> {
        let mut boa_context = Context::default();
        let app_context = init_bridge(&mut boa_context, renderer, font_loader);

        tracing::info!("TurApp initialized");

        Ok(TurApp {
            boa_context,
            app_context,
        })
    }

    pub fn load_js(&mut self, source: &str) -> Result<(), TurError> {
        self.boa_context
            .eval(Source::from_bytes(source))
            .map_err(TurError::JsEval)?;
        Ok(())
    }

    pub fn push_event(&self, event: AppEvent) {
        self.app_context.borrow_mut().push_event(event);
    }

    pub fn tick(&mut self) -> Result<(), TurError> {
        let mut needs_draw = false;

        loop {
            let events = self.app_context.borrow_mut().drain_events();
            if events.is_empty() {
                break;
            }

            for event in events {
                match event {
                    AppEvent::Resize {
                        logical_width,
                        logical_height,
                        dpr,
                    } => {
                        self.app_context.borrow_mut().resize_renderer(
                            logical_width,
                            logical_height,
                            dpr,
                        );
                        self.app_context
                            .borrow_mut()
                            .set_size(logical_width as f64, logical_height as f64);
                        needs_draw = true;
                    }

                    AppEvent::Gesture(AppGestureEvent::PointerDown { position }) => {
                        let target = self.app_context.borrow().hit_test(position);
                        self.app_context.borrow_mut().compose_pointer_down(target);

                        if let Some(id) = target {
                            let local = self.app_context.borrow().local_position(id, position);
                            self.app_context.borrow_mut().handle_gesture_event(
                                id,
                                &ComposedGestureEvent::PointerDown { local_position: local },
                            );
                        }
                    }

                    AppEvent::Gesture(AppGestureEvent::PointerMove { position }) => {
                        let (is_dragging, focused) = {
                            let ctx = self.app_context.borrow();
                            let dragging = ctx.is_gesture_dragging();
                            let focused = ctx.focused_element();
                            (dragging, focused)
                        };

                        if is_dragging {
                            if let Some(id) = focused {
                                let local = self.app_context.borrow().local_position(id, position);
                                self.app_context.borrow_mut().handle_gesture_event(
                                    id,
                                    &ComposedGestureEvent::PointerMove { local_position: local },
                                );
                            }
                        }
                    }

                    AppEvent::Gesture(AppGestureEvent::PointerUp { position }) => {
                        let click_eligible = {
                            let ctx = self.app_context.borrow();
                            let down_target = ctx.gesture_pointer_down_target();
                            match down_target {
                                Some(id) => ctx.hit_test_contains(position, id),
                                None => false,
                            }
                        };
                        let clicked = self
                            .app_context
                            .borrow_mut()
                            .compose_pointer_up(click_eligible);

                        if clicked {
                            let hit_path = {
                                let ctx = self.app_context.borrow();
                                ctx.element_tree().hit_test_path(position)
                            };

                            let focusable_id =
                                self.app_context.borrow().find_focusable_in_path(&hit_path);

                            if let Some(new_focused) = focusable_id {
                                let old_focused =
                                    self.app_context.borrow_mut().request_focus(new_focused);
                                if let Some(old) = old_focused {
                                    if old != new_focused {
                                        self.app_context.borrow_mut().dispatch_blur(old);
                                        self.app_context.borrow_mut().push_js_event_blur(old);
                                    }
                                }
                                self.app_context.borrow_mut().dispatch_focus(new_focused);
                                self.app_context.borrow_mut().push_js_event_focus(new_focused);
                            } else {
                                let old_focused = self.app_context.borrow_mut().clear_focus();
                                if let Some(old) = old_focused {
                                    self.app_context.borrow_mut().dispatch_blur(old);
                                    self.app_context.borrow_mut().push_js_event_blur(old);
                                }
                            }

                            for node_id in &hit_path {
                                self.app_context.borrow_mut().push_js_event_click(
                                    *node_id,
                                    position.x,
                                    position.y,
                                );
                            }
                        }
                    }

                    AppEvent::Key(key_event) => {
                        let result = self
                            .app_context
                            .borrow_mut()
                            .handle_key_event(&key_event);

                        if matches!(result, core::elements::KeyboardResult::NotHandled) {
                            let focused_id = self.app_context.borrow().focused_element();
                            if let Some(focused_id) = focused_id {
                                let mut current = Some(focused_id);
                                while let Some(id) = current {
                                    self.app_context
                                        .borrow_mut()
                                        .push_js_event_key_down(id, &key_event);
                                    current = self.app_context.borrow().element_tree().parent_of(id);
                                }
                            }
                        }
                    }

                    AppEvent::RequestDraw => {
                        needs_draw = true;
                    }
                }
            }
        }

        // Call phase: drain JsEventQueue and flush to elements
        // Take element out temporarily to avoid RefCell conflict when
        // JS callbacks re-enter the bridge during flush.
        let entries = self.app_context.borrow_mut().js_event_queue_mut().drain();
        for (target, event) in entries {
            let mut element = {
                let mut ctx = self.app_context.borrow_mut();
                ctx.element_tree_mut()
                    .get_mut(target)
                    .and_then(|n| n.element.take())
            };
            if let Some(ref mut element) = element {
                element.flush_js_event(event, &mut self.boa_context);
            }
            if element.is_some() {
                self.app_context
                    .borrow_mut()
                    .element_tree_mut()
                    .get_mut(target)
                    .unwrap()
                    .element = element;
            }
        }

        if needs_draw {
            self.app_context.borrow_mut().render();
            self.app_context
                .borrow_mut()
                .present_renderer()
                .map_err(|e| TurError::Render(e.to_string()))?;
        }

        Ok(())
    }

    pub fn debug_layout(&self) -> String {
        self.app_context.borrow().debug_layout()
    }

    pub fn query_element(&self, key: &[&str]) -> Option<ElementNodeId> {
        self.app_context.borrow().element_tree().query_element(key)
    }

    pub fn focused_element(&self) -> Option<ElementNodeId> {
        self.app_context.borrow().focused_element()
    }

    pub fn with_element<R>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        let ctx = self.app_context.borrow();
        let node = ctx.element_tree().get(id)?;
        let element = node.element.as_ref()?;
        Some(cb(element))
    }

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> std::cell::Ref<'_, ElementTree> {
        std::cell::Ref::map(self.app_context.borrow(), |ctx| ctx.element_tree())
    }
}
