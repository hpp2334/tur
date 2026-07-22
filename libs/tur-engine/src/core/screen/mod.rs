//! Screen domain — the canvas's logical size + the resize subsystem that
//! drives it.
//!
//! Owns:
//! - the current logical size (CSS pixels),
//! - the shared reactive `Store` (so the size→atom sync is self-contained),
//! - the `viewportSize$` reactive source atom that publishes `{width, height}`
//!   to JS,
//! - the [`ResizeSubsystem`] that handles `PlatformEvent::Resize` (resizes the
//!   renderer, updates the size, pushes the atom, and re-marks the tree root
//!   dirty).
//!
//! `TurAppContext` owns a [`Screen`] inline; `SubsystemFlushContext.screen`
//! is a `&mut` borrow into it, so the resize handler drives both the size
//! mutation and the atom sync directly (event-driven, not polled each frame).

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
    /// The shared reactive store, captured at construction so the resize
    /// handler can push the `viewportSize$` atom directly via
    /// [`Self::sync_source`] without the caller threading a `&Store` through
    /// `SubsystemFlushContext`. Cheap to clone (`Rc`-backed); observes the
    /// same reactive state engine-wide.
    pub(crate) store: Store,
    /// The reactive source atom that publishes `{width, height}` to JS.
    /// `None` until `TurEngineBuilder::build` creates it.
    pub(crate) source: Option<Source<JsValue>>,
    /// Last `(width, height)` pushed into `source` — guards against
    /// spurious stale marking (`set_source` compares `JsValue` by object
    /// identity, so a fresh `{w,h}` object would otherwise dirty on every
    /// push).
    last: Cell<(f64, f64)>,
}

impl Screen {
    /// Create with the given reactive store and the default initial logical
    /// size (400×600) — matches the historical `TurAppContext::new` default
    /// before this type existed.
    pub fn new(store: Store) -> Self {
        Self {
            logical_size: (400.0, 600.0),
            store,
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
    /// changed since the last sync. Called by [`ResizeSubsystem`] from its
    /// `PlatformEvent::Resize` handler (event-driven, not once per `flush()`
    /// iteration), so `viewportSize$` subscribers re-layout in the same
    /// fixed-point iteration that mutated [`Self::logical_size`].
    pub(crate) fn sync_source(&self, boa: &mut Context) {
        let Some(src) = self.source else {
            return;
        };
        let (width, height) = self.logical_size;
        if (width, height) == self.last.get() {
            return;
        }
        self.last.set((width, height));
        let value = Self::size_js(width, height, boa);
        self.store.bridge().set_source(src, value);
    }
}
