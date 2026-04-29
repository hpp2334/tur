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
use core::focus::FocusEventType;
use core::fonts::FontLoader;
use core::gesture::ComposedGestureEventKind;

pub struct TurApp {
    boa_context: Context,
    app_context: Rc<RefCell<TurAppInternal>>,
}

impl TurApp {
    pub fn new(
        renderer: Box<dyn core::render::Renderer>,
        font_loader: Box<dyn FontLoader>,
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
                                &mut self.boa_context,
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
                                    &mut self.boa_context,
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
                        let kind = self
                            .app_context
                            .borrow_mut()
                            .compose_pointer_up(click_eligible);
                        if let Some(kind) = kind {
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
                                        let cb = self
                                            .app_context
                                            .borrow()
                                            .collect_focus_handler(old, FocusEventType::Blur);
                                        if let Some(callback) = cb {
                                            let _ = callback.call(
                                                &boa_engine::JsValue::undefined(),
                                                &[],
                                                &mut self.boa_context,
                                            );
                                        }
                                    }
                                }
                                let cb = self
                                    .app_context
                                    .borrow()
                                    .collect_focus_handler(new_focused, FocusEventType::Focus);
                                if let Some(callback) = cb {
                                    let _ = callback.call(
                                        &boa_engine::JsValue::undefined(),
                                        &[],
                                        &mut self.boa_context,
                                    );
                                }
                            } else {
                                let old_focused = self.app_context.borrow_mut().clear_focus();
                                if let Some(old) = old_focused {
                                    let cb = self
                                        .app_context
                                        .borrow()
                                        .collect_focus_handler(old, FocusEventType::Blur);
                                    if let Some(callback) = cb {
                                        let _ = callback.call(
                                            &boa_engine::JsValue::undefined(),
                                            &[],
                                            &mut self.boa_context,
                                        );
                                    }
                                }
                            }

                            let callbacks = self
                                .app_context
                                .borrow()
                                .collect_event_handlers(kind, position);
                            for callback in callbacks {
                                let _ = callback.call(
                                    &boa_engine::JsValue::undefined(),
                                    &[],
                                    &mut self.boa_context,
                                );
                            }
                        }
                    }

                    AppEvent::Key(key_event) => {
                        self.app_context
                            .borrow_mut()
                            .handle_key_event(&key_event, &mut self.boa_context);
                    }

                    AppEvent::RequestDraw => {
                        needs_draw = true;
                    }
                }
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

    pub fn has_event_handler(&self, id: ElementNodeId, kind: ComposedGestureEventKind) -> bool {
        self.app_context.borrow().has_event_handler(id, kind)
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
