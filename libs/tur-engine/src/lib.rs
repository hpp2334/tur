pub mod core;
pub mod elements;
pub mod handlers;
pub mod renderer;

pub mod error;

use std::rc::Rc;
use std::time::Duration;

use boa_engine::context::time::FixedClock;
use boa_engine::Context;
use boa_engine::Source;
use std::path::Path;
use error::TurError;

use core::app::TurAppInternal;
use core::bridge::init_bridge;
use core::bridge::TurJobExecutor;
use core::element::ElementNodeId;
use core::elements::AnyElement;
#[cfg(feature = "trace")]
use core::elements::ElementTree;
use elements::editable_text::EditableTextElement;

pub struct TurApp {
    boa_context: Context,
    internal: TurAppInternal,
    clock: Rc<FixedClock>,
    executor: Rc<TurJobExecutor>,
}

impl TurApp {
    pub fn new(
        renderer: Box<dyn core::render::Renderer>,
        font_loader: Box<dyn core::fonts::FontLoader>,
    ) -> Result<Self, TurError> {
        let clock = Rc::new(FixedClock::from_millis(0));
        let executor = Rc::new(TurJobExecutor::new());
        let mut boa_context = Context::builder()
            .clock(clock.clone())
            .job_executor(executor.clone())
            .build()
            .expect("failed to build boa context");

        let BridgeResult { internal, clock, executor } =
            init_bridge(&mut boa_context, renderer, font_loader, clock, executor);

        tracing::info!("TurApp initialized");

        Ok(TurApp {
            boa_context,
            internal,
            clock,
            executor,
        })
    }

    pub fn load_js(&mut self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_js: evaluating bundle ({} bytes)", source.len());
        self.boa_context
            .eval(Source::from_bytes(source).with_path(Path::new("bundle.js")))
            .map_err(|e| {
                tracing::error!("JS eval error: {e}");
                TurError::JsEval(e)
            })?;
        if let Err(e) = self.executor.drain(&mut self.boa_context) {
            tracing::error!("load_js drain error: {e}");
        }
        tracing::info!("load_js: bundle evaluated successfully");
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

    pub fn spawn_loop_once(&mut self, advanced_time: Duration) -> Result<(), TurError> {
        self.clock.forward(advanced_time.as_millis() as u64);
        self.internal.flush(&mut self.boa_context)?;
        Ok(())
    }

    pub fn push_event(&self, event: core::event::AppEvent) {
        self.internal
            .app_context
            .borrow_mut()
            .event_queue
            .push(event);
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
        let editable_el = element.cast::<EditableTextElement>()?;
        let layout_data = editable_el.cached_layout.as_ref()?;
        let cursor_pos = editable_el.cursor_position?;

        let (cursor_x, _) = layout_data.cursor_xy_at(cursor_pos);
        let line_idx = layout_data.line_index_for_char(cursor_pos);
        let line_info = &layout_data.line_infos[line_idx];

        Some((
            abs_x + cursor_x as f64,
            abs_y + line_info.top as f64,
            2.0,
            line_info.height as f64,
        ))
    }

    pub fn focused_is_editable(&self) -> bool {
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
        element.cast::<EditableTextElement>().is_some()
    }

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> std::cell::Ref<'_, ElementTree> {
        std::cell::Ref::map(self.internal.js_context.element_tree.borrow(), |t| t)
    }

    pub fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        self.internal.app_context.borrow_mut().render_to_pixels()
    }
}

use core::bridge::BridgeResult;
