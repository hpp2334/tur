use crate::core::edgy::reactive::{ReactiveBridgeStore, Source};
use crate::core::edgy::value::Value;
use crate::core::platform::PlatformEvent;
use crate::core::shell::ShellEvent;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

/// Publishes shell `Resize` events into the `viewport_width` /
/// `viewport_height` atoms.
///
/// This is the canonical engine-atom pattern (the same shape as the
/// `CounterSubsystem` plugin recipe): the subsystem owns the two **backing**
/// sources — their single value home is the instance store — and the
/// instance store's bridge (the ordinary `set_source` write rail). The
/// public faces for rut code are the `env` rows' atom ids
/// (`env_viewport_w` / `env_viewport_h`, resolved through the
/// `ViewportEnv` plugin state), minted together with this subsystem by
/// `TurStdPlugin::register`. [`crate::core::screen::Screen`]
/// stays pure data: only the logical size and dpr live there.
///
/// Engine infrastructure that the std plugin wires: `TurStdPlugin` registers
/// it FIRST in dispatch order (before gesture / keyboard / ime /
/// pointer-region), so the atoms exist before anything can read them.
pub struct ResizeSubsystem {
    /// The `viewport_width` backing source (CSS-pixel logical f64).
    backing_w: Source<f64>,
    /// The `viewport_height` backing source (CSS-pixel logical f64).
    backing_h: Source<f64>,
    /// The instance store's write rail.
    bridge: ReactiveBridgeStore,
    /// Last `(width, height)` pushed into the backings — guards against
    /// spurious stale marking (without this guard every resize would dirty
    /// the atoms even at an unchanged size).
    last: (f64, f64),
}

impl ResizeSubsystem {
    /// Construct with the backings + the instance store's bridge.
    /// `initial` is the size already carried by the backings' seeds (the
    /// engine builder mints it from the real viewport), so a first resize
    /// to that same size dedups — the atoms already hold the value.
    pub(crate) fn new(
        backing_w: Source<f64>,
        backing_h: Source<f64>,
        bridge: ReactiveBridgeStore,
        initial: (f64, f64),
    ) -> Self {
        Self {
            backing_w,
            backing_h,
            bridge,
            last: initial,
        }
    }
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
        // Publish the new size into the two viewport backings through the
        // instance store's write rail, so subscribers re-layout in this
        // same fixed-point iteration. `set_source`-with-equal-size dedups
        // via `last` (the pair moves together — one guard). Pre-mount the
        // tree is simply rootless — `mark_root_dirty` is a no-op then.
        //
        // Realm-free: the atoms' values are plain f64s, so the write never
        // touches any script realm.
        if size != self.last {
            self.last = size;
            // A resize can never be a watch loop (it originates from the
            // platform event queue, never inside a watcher callback
            // delivery), so an error here would be an engine invariant
            // violation — log, don't crash.
            if let Err(e) = self.bridge.set_source(self.backing_w, Value::Num(size.0)) {
                tracing::error!("viewport_width sync failed: {e}");
            }
            if let Err(e) = self.bridge.set_source(self.backing_h, Value::Num(size.1)) {
                tracing::error!("viewport_height sync failed: {e}");
            }
        }
        cx.element_tree.borrow_mut().mark_root_dirty();
        cx.request_paint();
    }
}
