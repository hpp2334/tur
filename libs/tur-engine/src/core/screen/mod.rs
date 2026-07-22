//! Screen domain — the canvas's logical size + the resize subsystem that
//! drives it.
//!
//! Owns:
//! - the current `screen_logical_size` (CSS pixels),
//! - the `viewportSize$` reactive source atom that publishes `{width, height}`
//!   to JS,
//! - the [`ResizeSubsystem`] that handles `PlatformEvent::Resize` (forwards
//!   to the renderer + re-marks the tree root dirty).
//!
//! `TurAppContext` owns a [`Screen`] inline; `SubsystemFlushContext.screen_logical_size`
//! is a `&mut` borrow into `Screen::logical_size`, so subsystems read and
//! write the size via `cx.screen_logical_size`.

pub mod resize;

pub use resize::ResizeSubsystem;

use std::cell::Cell;

use boa_engine::{Context, JsValue, js_string, object::JsObject};

use crate::core::edgy::reactive::{Source, Store};

/// Engine screen state — the canvas's logical size + the `viewportSize$`
/// reactive source atom that publishes it to JS.
pub struct Screen {
    /// Current canvas logical size, in CSS pixels. Updated by
    /// [`ResizeSubsystem`] when a `PlatformEvent::Resize` arrives.
    pub logical_size: (f64, f64),
    /// The reactive source atom that publishes `{width, height}` to JS.
    /// `None` until `TurEngineBuilder::build` creates it.
    pub(crate) source: Option<Source<JsValue>>,
    /// Last `(width, height)` pushed into `source` — guards against
    /// spurious stale marking (`set_source` compares `JsValue` by object
    /// identity, so a fresh `{w,h}` object would otherwise dirty every
    /// frame).
    last: Cell<(f64, f64)>,
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen {
    /// Create with the default initial logical size (400×600) — matches
    /// the historical `TurAppContext::new` default before this type
    /// existed.
    pub fn new() -> Self {
        Self {
            logical_size: (400.0, 600.0),
            source: None,
            last: Cell::new((-1.0, -1.0)),
        }
    }

    /// Build a `{width, height}` JS object (CSS pixels) — the value shape
    /// of the `viewportSize$` atom. Consumed by the engine builder (initial
    /// const value) and [`Self::sync_source`] (per-resize update).
    pub(crate) fn size_js(width: f64, height: f64, boa: &mut Context) -> JsValue {
        let obj = JsObject::with_object_proto(boa.intrinsics());
        let _ = obj.create_data_property(js_string!("width"), JsValue::from(width), boa);
        let _ = obj.create_data_property(js_string!("height"), JsValue::from(height), boa);
        obj.into()
    }

    /// Install the `viewportSize$` source atom. Called once from
    /// `TurEngineBuilder::build` after the `Source` is created on the
    /// reactive store.
    pub(crate) fn set_source(&mut self, src: Source<JsValue>) {
        self.last.set((-1.0, -1.0));
        self.source = Some(src);
    }

    /// Push the current logical size into the source atom if it has
    /// changed since the last sync. Called once per `flush()` (before the
    /// reactive flush) so subscribers re-layout in-frame.
    pub(crate) fn sync_source(&self, store: &Store, boa: &mut Context) {
        let Some(src) = self.source else {
            return;
        };
        let (width, height) = self.logical_size;
        if (width, height) == self.last.get() {
            return;
        }
        self.last.set((width, height));
        let value = Self::size_js(width, height, boa);
        store.bridge().set_source(src, value);
    }
}
