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
use elements::input::InputElement;

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

    pub fn eval_js(&mut self, source: &str) -> Result<String, TurError> {
        let result = self
            .boa_context
            .eval(Source::from_bytes(source))
            .map_err(TurError::JsEval)?;
        let s = result
            .as_string()
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|| result.display().to_string());
        Ok(s)
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

    pub fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let focused_id = self.focused_element()?;
        let tree = self.internal.js_context.element_tree.borrow();

        let mut abs_x = 0.0f64;
        let mut abs_y = 0.0f64;
        let mut current = Some(focused_id);
        while let Some(id) = current {
            let node = tree.get(id)?;
            abs_x += node.computed_layout.offset.x;
            abs_y += node.computed_layout.offset.y;
            current = node.parent;
        }

        let node = tree.get(focused_id)?;
        let element = node.element.as_ref()?;
        let input_el = element.cast::<InputElement>()?;
        let layout_data = input_el.cached_layout.as_ref()?;

        let effective_cursor = if let Some(ref comp) = input_el.composition_text {
            input_el.composition_start + comp.len()
        } else {
            input_el.cursor_position
        };

        let effective_text = input_el.composition_display_text();
        let char_idx = byte_to_char_offset(&effective_text, effective_cursor);
        let (cursor_x, cursor_y) = layout_data.cursor_xy_at(char_idx);
        let line_idx = layout_data.line_index_for_char(char_idx);
        let line_height = layout_data.line_height_at(line_idx);

        Some((
            abs_x + cursor_x as f64,
            abs_y + cursor_y as f64,
            2.0,
            line_height as f64,
        ))
    }

    pub fn focused_is_input(&self) -> bool {
        let Some(focused_id) = self.focused_element() else {
            return false;
        };
        let tree = self.internal.js_context.element_tree.borrow();
        let Some(node) = tree.get(focused_id) else {
            return false;
        };
        let Some(ref element) = node.element else {
            return false;
        };
        element.cast::<InputElement>().is_some()
    }

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> std::cell::Ref<'_, ElementTree> {
        std::cell::Ref::map(self.internal.js_context.element_tree.borrow(), |t| t)
    }
}

fn byte_to_char_offset(s: &str, byte_pos: usize) -> usize {
    s[..byte_pos].chars().count()
}
