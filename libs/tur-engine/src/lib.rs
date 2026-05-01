pub mod core;
pub mod elements;
pub mod renderer;

pub mod error;

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
    internal: TurAppInternal,
}

impl TurApp {
    pub fn new(
        renderer: Box<dyn core::render::Renderer>,
        font_loader: Box<dyn core::fonts::FontLoader>,
    ) -> Result<Self, TurError> {
        let mut boa_context = Context::default();
        let internal = init_bridge(&mut boa_context, renderer, font_loader);

        tracing::info!("TurApp initialized");

        Ok(TurApp {
            boa_context,
            internal,
        })
    }

    pub fn load_js(&mut self, source: &str) -> Result<(), TurError> {
        self.boa_context
            .eval(Source::from_bytes(source))
            .map_err(TurError::JsEval)?;
        Ok(())
    }

    pub fn push_event(&self, event: AppEvent) {
        self.internal.app_context.borrow_mut().event_queue.push(event);
    }

    pub fn tick(&mut self) -> Result<(), TurError> {
        let mut needs_draw = false;

        loop {
            let events = self.internal.app_context.borrow_mut().event_queue.drain();
            if events.is_empty() {
                break;
            }

            for event in events {
                self.internal.app_context.borrow_mut().dispatch_handlers(&event);

                match event {
                    AppEvent::Resize {
                        logical_width,
                        logical_height,
                        dpr,
                    } => {
                        let mut ctx = self.internal.app_context.borrow_mut();
                        ctx.renderer.resize(logical_width, logical_height, dpr);
                        ctx.size = (logical_width as f64, logical_height as f64);
                        needs_draw = true;
                    }

                    AppEvent::Gesture(AppGestureEvent::PointerDown { position }) => {
                        let target = {
                            let tree = self.internal.js_context.element_tree.borrow();
                            core::hit_test::HitTest::new(&tree).deepest(position)
                        };
                        self.internal
                            .app_context
                            .borrow_mut()
                            .gesture_composer
                            .on_pointer_down(target);

                        if let Some(id) = target {
                            let local =
                                self.internal.app_context.borrow().local_position(id, position);
                            self.internal.app_context.borrow_mut().handle_gesture_event(
                                id,
                                &ComposedGestureEvent::PointerDown { local_position: local },
                            );
                        }
                    }

                    AppEvent::Gesture(AppGestureEvent::PointerMove { position }) => {
                        let (is_dragging, focused) = {
                            let ctx = self.internal.app_context.borrow();
                            let dragging = ctx.gesture_composer.is_tracking_drag();
                            let focused = ctx.focus_manager.borrow().focused();
                            (dragging, focused)
                        };

                        if is_dragging {
                            if let Some(id) = focused {
                                let local = self
                                    .internal
                                    .app_context
                                    .borrow()
                                    .local_position(id, position);
                                self.internal.app_context.borrow_mut().handle_gesture_event(
                                    id,
                                    &ComposedGestureEvent::PointerMove { local_position: local },
                                );
                            }
                        }
                    }

                    AppEvent::Gesture(AppGestureEvent::PointerUp { position }) => {
                        let click_eligible = {
                            let ctx = self.internal.app_context.borrow();
                            let down_target = ctx.gesture_composer.pointer_down_target();
                            match down_target {
                                Some(id) => {
                                    let tree = ctx.element_tree.borrow();
                                    core::hit_test::HitTest::new(&tree).contains(position, id)
                                }
                                None => false,
                            }
                        };
                        let clicked = self
                            .internal
                            .app_context
                            .borrow_mut()
                            .gesture_composer
                            .on_pointer_up(click_eligible)
                            .is_some();

                        if clicked {
                            let hit_path = {
                                let tree = self.internal.js_context.element_tree.borrow();
                                core::hit_test::HitTest::new(&tree).path(position)
                            };

                            for node_id in &hit_path {
                                self.internal
                                    .js_context
                                    .js_command_queue
                                    .borrow_mut()
                                    .push(
                                        *node_id,
                                        core::gesture::make_click_command(position.x, position.y),
                                    );
                            }
                        }
                    }

                    AppEvent::Key(key_event) => {
                        self.internal
                            .app_context
                            .borrow_mut()
                            .handle_key_event(&key_event);

                        let focused_id =
                            self.internal.js_context.focus_manager.borrow().focused();
                        if let Some(focused_id) = focused_id {
                            let mut current = Some(focused_id);
                            while let Some(id) = current {
                                self.internal
                                    .js_context
                                    .js_command_queue
                                    .borrow_mut()
                                    .push(
                                        id,
                                        core::keyboard::make_key_down_command(&key_event),
                                    );
                                current = self
                                    .internal
                                    .js_context
                                    .element_tree
                                    .borrow()
                                    .parent_of(id);
                            }
                        }
                    }

                    AppEvent::RequestDraw => {
                        needs_draw = true;
                    }
                }
            }
        }

        let mut pending_callbacks: Vec<(boa_engine::object::JsObject, Vec<boa_engine::JsValue>)> =
            Vec::new();

        loop {
            let entries = self.internal.js_context.js_command_queue.borrow_mut().drain();
            if entries.is_empty() {
                break;
            }
            for (target, command) in entries {
                let tree = self.internal.js_context.element_tree.borrow();
                if let Some(node) = tree.get(target) {
                    if let Some(ref element) = node.element {
                        if let Some(pair) = element.emit_js_callback(&mut self.boa_context, command)
                        {
                            pending_callbacks.push(pair);
                        }
                    }
                }
            }
        }

        for (callback, args) in pending_callbacks {
            let _ = callback.call(&boa_engine::JsValue::undefined(), &args, &mut self.boa_context);
        }

        let dirty = self.internal.js_context.dirty.take();
        if needs_draw || dirty {
            self.internal.app_context.borrow_mut().render();
            self.internal
                .app_context
                .borrow_mut()
                .renderer
                .present()
                .map_err(|e| TurError::Render(e.to_string()))?;
        }

        Ok(())
    }

    pub fn debug_layout(&self) -> String {
        self.internal.js_context.element_tree.borrow().debug_layout()
    }

    pub fn query_element(&self, key: &[&str]) -> Option<ElementNodeId> {
        self.internal
            .js_context
            .element_tree
            .borrow()
            .query_element(key)
    }

    pub fn focused_element(&self) -> Option<ElementNodeId> {
        self.internal.js_context.focus_manager.borrow().focused()
    }

    pub fn with_element<R>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        let tree = self.internal.js_context.element_tree.borrow();
        let node = tree.get(id)?;
        let element = node.element.as_ref()?;
        Some(cb(element))
    }

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> std::cell::Ref<'_, ElementTree> {
        std::cell::Ref::map(self.internal.js_context.element_tree.borrow(), |t| t)
    }
}
