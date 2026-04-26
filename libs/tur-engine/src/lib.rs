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
use core::event::{AppEvent, AppGestureEvent};
pub use core::fonts::{FontLoader, PresetFontLoader};
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

    pub fn app_context(&self) -> &Rc<RefCell<TurAppInternal>> {
        &self.app_context
    }

    pub fn push_event(&self, event: AppEvent) {
        self.app_context.borrow().push_event(event);
    }

    pub fn tick(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut needs_draw = false;

        loop {
            let events = self.app_context.borrow().drain_events();
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
                        self.app_context.borrow().renderer().borrow_mut().resize(
                            logical_width,
                            logical_height,
                            dpr,
                        );
                        self.app_context
                            .borrow()
                            .set_size(logical_width as f64, logical_height as f64);
                        needs_draw = true;
                    }

                    AppEvent::Gesture(AppGestureEvent::PointerDown { position }) => {
                        let target = self.app_context.borrow().hit_test(position);
                        self.app_context.borrow().compose_pointer_down(target);
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
                        if let Some(kind) =
                            self.app_context.borrow().compose_pointer_up(click_eligible)
                        {
                            self.app_context.borrow().invoke_handlers_for(
                                kind,
                                position,
                                &mut self.boa_context,
                            );
                        }
                    }

                    AppEvent::RequestDraw => {
                        needs_draw = true;
                    }
                }
            }
        }

        if needs_draw {
            self.app_context.borrow().render();
            self.app_context
                .borrow()
                .renderer()
                .borrow_mut()
                .present()?;
        }

        Ok(())
    }

    pub fn debug_layout(&self) -> String {
        self.app_context
            .borrow()
            .element_tree()
            .borrow()
            .debug_layout()
    }

    pub fn has_event_handler(&self, id: ElementNodeId, kind: ComposedGestureEventKind) -> bool {
        self.app_context.borrow().has_event_handler(id, kind)
    }

    pub fn query_element(&self, key: &[&str]) -> Option<ElementNodeId> {
        self.app_context
            .borrow()
            .element_tree()
            .borrow()
            .query_element(key)
    }

    pub fn with_element<R>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        let ctx = self.app_context.borrow();
        let tree = ctx.element_tree().borrow();
        let node = tree.get(id)?;
        let element = node.element.as_ref()?;
        Some(cb(element))
    }

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> Rc<RefCell<ElementTree>> {
        self.app_context.borrow().element_tree_rc()
    }
}
