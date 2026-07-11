pub mod bridge;
pub mod elements;
pub mod handlers;
pub mod keyboard;
pub mod platform;
pub mod pointer_region;
pub mod scroll;
pub mod text;

use std::cell::RefCell;
use std::rc::Rc;

use tur_engine::core::bridge::helpers::{ConstEntry, FnEntry};
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::core::bridge::{reactive, render};
use tur_engine::error::TurError;
use tur_shared::Cursor;

pub use platform::{ClipboardPlatform, CursorPlatform, NoopClipboardPlatform, NoopCursorPlatform};

pub struct TurStdPlugin {
    cursor: Rc<RefCell<dyn CursorPlatform>>,
    #[allow(dead_code)]
    clipboard: Rc<RefCell<dyn ClipboardPlatform>>,
}

impl TurStdPlugin {
    pub fn builder() -> TurStdPluginBuilder {
        TurStdPluginBuilder::new()
    }
}

impl Default for TurStdPlugin {
    fn default() -> Self {
        Self {
            cursor: Rc::new(RefCell::new(NoopCursorPlatform)),
            clipboard: Rc::new(RefCell::new(NoopClipboardPlatform)),
        }
    }
}

impl Plugin for TurStdPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        use tur_engine::core::animation::AnimationController;
        use crate::scroll::ScrollController;
        use crate::text::{TextEditingController, UndoController};
        use crate::elements::lazy_list::LazyListController;

        ctx.register_class::<TextEditingController>()
            .expect("failed to register TextEditingController");
        ctx.register_class::<UndoController>()
            .expect("failed to register UndoController");
        ctx.register_class::<ScrollController>()
            .expect("failed to register ScrollController");
        ctx.register_class::<LazyListController>()
            .expect("failed to register LazyListController");
        ctx.register_class::<AnimationController>()
            .expect("failed to register AnimationController");

        ctx.register_handler(Box::new(handlers::gesture::GestureAppHandler::new()));
        ctx.register_handler(Box::new(handlers::keyboard::KeyboardAppHandler));
        ctx.register_handler(Box::new(handlers::ime::ImeAppHandler));
        ctx.register_handler(Box::new(handlers::resize::ResizeHandler));
        ctx.register_handler(Box::new(handlers::pointer_region::PointerRegionAppHandler::new()));
        ctx.register_handler(Box::new(handlers::wheel::WheelAppHandler));
        ctx.register_handler(Box::new(handlers::scroll_chaining::ScrollChainingHandler));
        ctx.register_handler(Box::new(handlers::scroll_to::ScrollToHandler));
        ctx.register_handler(Box::new(handlers::clipboard::ClipboardPasteHandler));
        ctx.register_handler(Box::new(handlers::clipboard::ClipboardWriteHandler));

        let mut std_fns: Vec<FnEntry> = Vec::new();
        std_fns.extend(reactive::fns());
        std_fns.extend(render::fns());
        std_fns.extend(bridge::color::fns());
        std_fns.extend(bridge::animation::fns());
        std_fns.extend(tur_engine::elements::container::bridge::fns());
        std_fns.extend(tur_engine::elements::flex::bridge::fns());
        std_fns.extend(tur_engine::elements::flex_item::bridge::fns());
        std_fns.extend(tur_engine::elements::stack::bridge::fns());
        std_fns.extend(tur_engine::elements::positioned::bridge::fns());
        std_fns.extend(crate::elements::paragraph::bridge::fns());
        std_fns.extend(crate::elements::editable_text::bridge::fns());
        std_fns.extend(tur_engine::elements::image::bridge::fns());
        std_fns.extend(tur_engine::elements::pointer_interact::bridge::fns());
        std_fns.extend(tur_engine::elements::mouse_region::bridge::fns());
        std_fns.extend(tur_engine::elements::condition::bridge::fns());
        std_fns.extend(tur_engine::elements::switch::bridge::fns());
        std_fns.extend(tur_engine::elements::each::bridge::fns());
        std_fns.extend(crate::elements::lazy_list::bridge::fns());
        std_fns.extend(crate::elements::scroll_view::bridge::fns());
        std_fns.extend(crate::elements::scrollbar::bridge::fns());
        std_fns.extend(tur_engine::elements::fragment::bridge::fns());
        std_fns.extend(crate::elements::focusable::bridge::fns());
        std_fns.extend(tur_engine::elements::effects::bridge::fns());
        std_fns.extend(tur_engine::elements::lifecycle::bridge::fns());
        std_fns.extend(tur_engine::elements::readable_subscribe::bridge::fns());

        let mut std_consts: Vec<ConstEntry> = Vec::new();
        let js_ctx_value = ctx.js_ctx_value.clone();
        std_consts.extend(bridge::color::consts(ctx.boa_mut(), js_ctx_value));
        std_consts.extend(bridge::enums::consts(ctx.boa_mut()));
        // Engine-owned reactive source exposing the live canvas size as
        // `{width, height}` (CSS pixels). The engine syncs it each frame in
        // `TurAppInternal::flush`; JS reads it via `get(viewportSize$).width`.
        std_consts.push(("viewportSize$", ctx.viewport_size.clone()));

        ctx.register_module("builtin:tur/std", std_fns, std_consts);

        Ok(())
    }

    fn cursor_output(&self) -> Option<Box<dyn FnMut(Cursor)>> {
        let cursor = self.cursor.clone();
        Some(Box::new(move |c| cursor.borrow_mut().set_cursor(c)))
    }
}

pub struct TurStdPluginBuilder {
    cursor: Rc<RefCell<dyn CursorPlatform>>,
    #[allow(dead_code)]
    clipboard: Rc<RefCell<dyn ClipboardPlatform>>,
}

impl Default for TurStdPluginBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TurStdPluginBuilder {
    pub fn new() -> Self {
        Self {
            cursor: Rc::new(RefCell::new(NoopCursorPlatform)),
            clipboard: Rc::new(RefCell::new(NoopClipboardPlatform)),
        }
    }

    pub fn cursor<P: CursorPlatform + 'static>(mut self, platform: P) -> Self {
        self.cursor = Rc::new(RefCell::new(platform));
        self
    }

    pub fn clipboard<P: ClipboardPlatform + 'static>(mut self, platform: P) -> Self {
        self.clipboard = Rc::new(RefCell::new(platform));
        self
    }

    pub fn build(self) -> TurStdPlugin {
        TurStdPlugin {
            cursor: self.cursor,
            clipboard: self.clipboard,
        }
    }
}
