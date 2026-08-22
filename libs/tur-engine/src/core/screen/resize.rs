use boa_engine::{Context, JsObject, JsValue, js_string};

use crate::core::edgy::reactive::{ReactiveBridgeStore, Source};
use crate::core::platform::PlatformEvent;
use crate::core::shell::ShellEvent;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

/// Publishes shell `Resize` events into the `viewportSize$` atom.
///
/// This is the canonical engine-atom pattern (the same shape as the
/// `CounterSubsystem` plugin recipe): the subsystem owns the **backing**
/// source — its single value home is the instance store — and the instance
/// store's bridge (the ordinary `set_source` write rail). The public
/// `viewportSize$` handle is a derive over the backing, minted together
/// with this subsystem by `TurStdPlugin::register`. [`crate::core::screen::Screen`]
/// stays pure data: only the logical size and dpr live there.
///
/// Engine infrastructure that the std plugin wires: `TurStdPlugin` registers
/// it FIRST in dispatch order (before gesture / keyboard / ime /
/// pointer-region), so the atom exists before anything can read it.
pub struct ResizeSubsystem {
    /// The `viewportSize$` backing source (the public handle is a derive
    /// over this).
    backing: Source<JsValue>,
    /// The instance store's write rail.
    bridge: ReactiveBridgeStore,
    /// Last `(width, height)` pushed into `backing` — guards against
    /// spurious stale marking (`set_source` compares `JsValue`s by object
    /// identity, so a fresh `{w,h}` object would otherwise dirty on every
    /// push).
    last: (f64, f64),
}

impl ResizeSubsystem {
    /// Construct with the backing + the instance store's bridge. `initial`
    /// is the size already carried by the backing's seed (the engine
    /// builder mints it from the real viewport), so a first resize to that
    /// same size dedups — the atom already holds the value.
    pub(crate) fn new(
        backing: Source<JsValue>,
        bridge: ReactiveBridgeStore,
        initial: (f64, f64),
    ) -> Self {
        Self {
            backing,
            bridge,
            last: initial,
        }
    }
}

/// Build the `{width, height}` JS object (CSS pixels) — the value shape of
/// the `viewportSize$` atom (its seed in the engine builder, its per-resize
/// payload here).
pub(crate) fn viewport_size_value(width: f64, height: f64, boa: &mut Context) -> JsValue {
    let obj = JsObject::with_object_proto(boa.intrinsics());
    let _ = obj.create_data_property(js_string!("width"), JsValue::from(width), boa);
    let _ = obj.create_data_property(js_string!("height"), JsValue::from(height), boa);
    obj.into()
}

impl Subsystem for ResizeSubsystem {
    fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent) {
        let PlatformEvent::Shell(ShellEvent::Resize {
            logical_width,
            logical_height,
            dpr,
        }) = event
        else {
            return;
        };

        // The renderer lives on the host thread; the embedder resizes it directly at
        // event-receipt time via `TurApp::resize` (which also forwards this
        // shell `Resize` event to the worker). Here we only update the
        // worker-side screen state.
        let size = (*logical_width as f64, *logical_height as f64);
        cx.screen.logical_size = size;
        cx.screen.dpr = *dpr;
        // Publish the new size into the `viewportSize$` backing through the
        // instance store's write rail, so subscribers re-layout in this
        // same fixed-point iteration. `set_source`-with-equal-size dedups
        // via `last`. Pre-mount the tree is simply rootless —
        // `mark_root_dirty` is a no-op then.
        if size != self.last {
            self.last = size;
            let value = viewport_size_value(size.0, size.1, cx.boa);
            // A resize can never be a watch loop (it originates from the
            // platform event queue, never inside a watcher callback
            // delivery), so an error here would be an engine invariant
            // violation — log, don't crash.
            if let Err(e) = self.bridge.set_source(self.backing, value) {
                tracing::error!("viewportSize$ sync failed: {e}");
            }
        }
        cx.element_tree.borrow_mut().mark_root_dirty();
        cx.request_paint();
    }
}
