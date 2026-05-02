pub mod core;
pub mod elements;
pub mod handlers;
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

    pub fn push_event(&self, event: core::event::AppEvent) {
        self.internal.app_context.borrow_mut().event_queue.push(event);
    }

    pub fn tick(&mut self) -> Result<(), TurError> {
        self.internal.flush(&mut self.boa_context)
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
