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

use crate::core::bridge::helpers::{ConstEntry, FnEntry};
use crate::core::plugin::{Plugin, PluginContext};
use crate::core::bridge::{reactive, render};
use crate::error::TurError;
use tur_shared::Cursor;

pub use platform::{Clipboard, CursorPlatform, NoopClipboard, NoopCursorPlatform};

pub struct TurStdPlugin {
    cursor: Rc<RefCell<dyn CursorPlatform>>,
    clipboard: Rc<dyn Clipboard>,
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
            clipboard: Rc::new(NoopClipboard),
        }
    }
}

impl Plugin for TurStdPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        use crate::core::animation::AnimationController;
        use crate::stdlib::scroll::ScrollController;
        use crate::stdlib::text::{TextEditingController, UndoController};
        use crate::stdlib::elements::lazy_list::LazyListController;

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
        ctx.register_handler(Box::new(handlers::clipboard::ClipboardWriteHandler::new(
            self.clipboard.clone(),
            ctx.async_executor().clone(),
        )));

        let mut std_fns: Vec<FnEntry> = Vec::new();
        std_fns.extend(reactive::fns());
        std_fns.extend(render::fns());
        std_fns.extend(crate::core::bridge::task::fns());
        std_fns.extend(bridge::color::fns());
        std_fns.extend(bridge::animation::fns());
        std_fns.extend(crate::elements::container::bridge::fns());
        std_fns.extend(crate::elements::flex::bridge::fns());
        std_fns.extend(crate::elements::flex_item::bridge::fns());
        std_fns.extend(crate::elements::stack::bridge::fns());
        std_fns.extend(crate::elements::positioned::bridge::fns());
        std_fns.extend(crate::stdlib::elements::paragraph::bridge::fns());
        std_fns.extend(crate::stdlib::elements::editable_text::bridge::fns());
        std_fns.extend(crate::elements::image::bridge::fns());
        std_fns.extend(crate::elements::pointer_interact::bridge::fns());
        std_fns.extend(crate::elements::mouse_region::bridge::fns());
        std_fns.extend(crate::elements::condition::bridge::fns());
        std_fns.extend(crate::elements::switch::bridge::fns());
        std_fns.extend(crate::elements::each::bridge::fns());
        std_fns.extend(crate::stdlib::elements::lazy_list::bridge::fns());
        std_fns.extend(crate::stdlib::elements::scroll_view::bridge::fns());
        std_fns.extend(crate::stdlib::elements::scrollbar::bridge::fns());
        std_fns.extend(crate::elements::fragment::bridge::fns());
        std_fns.extend(crate::stdlib::elements::focusable::bridge::fns());
        std_fns.extend(crate::elements::effects::bridge::fns());
        std_fns.extend(crate::elements::lifecycle::bridge::fns());
        std_fns.extend(crate::elements::readable_subscribe::bridge::fns());

        // Clipboard bridge moved to `builtin:tur/clipboard` (tur-clipboard
        // crate). The `Clipboard` trait + `ClipboardWriteHandler` /
        // `ClipboardPasteHandler` stay here for the engine's internal
        // Cmd+C/Cmd+V/Cmd+X event path. The JS-callable `clipboard.readText`
        // / `clipboard.writeText` fns are registered by `TurClipboardPlugin`.

        let mut std_consts: Vec<ConstEntry> = Vec::new();
        let js_ctx_value = ctx.js_ctx_value.clone();
        std_consts.extend(bridge::color::consts(ctx.boa_mut(), js_ctx_value));
        std_consts.extend(bridge::enums::consts(ctx.boa_mut()));
        // Engine-owned reactive source exposing the live canvas size as
        // `{width, height}` (CSS pixels). The engine syncs it each frame in
        // `TurAppInternal::flush`; JS reads it via `get(viewportSize$).width`.
        std_consts.push(("viewportSize$", ctx.viewport_size.clone()));

        ctx.register_module("builtin:tur/std", std_fns, vec![], std_consts);

        Ok(())
    }

    fn cursor_output(&self) -> Option<Box<dyn FnMut(Cursor)>> {
        let cursor = self.cursor.clone();
        Some(Box::new(move |c| cursor.borrow_mut().set_cursor(c)))
    }
}

pub struct TurStdPluginBuilder {
    cursor: Rc<RefCell<dyn CursorPlatform>>,
    clipboard: Rc<dyn Clipboard>,
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
            clipboard: Rc::new(NoopClipboard),
        }
    }

    pub fn cursor<P: CursorPlatform + 'static>(mut self, platform: P) -> Self {
        self.cursor = Rc::new(RefCell::new(platform));
        self
    }

    pub fn clipboard<P: Clipboard + 'static>(mut self, platform: P) -> Self {
        self.clipboard = Rc::new(platform);
        self
    }

    pub fn build(self) -> TurStdPlugin {
        TurStdPlugin {
            cursor: self.cursor,
            clipboard: self.clipboard,
        }
    }
}
