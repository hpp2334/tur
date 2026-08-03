use crate::core::platform::PlatformEvent;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

pub struct ResizeSubsystem;

impl Subsystem for ResizeSubsystem {
    fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent) {
        let PlatformEvent::Resize {
            logical_width,
            logical_height,
            dpr,
        } = event
        else {
            return;
        };

        // The actual renderer lives on main (worker→main command shipping).
        // Main-side `render_sink` callback receives the current viewport
        // tuple `(width, height, dpr)` with each `MainMsg::RenderCommands`
        // and is responsible for calling `renderer.resize(...)` when the
        // size changes. Here we only update the worker-side screen state.
        cx.screen.logical_size = (*logical_width as f64, *logical_height as f64);
        cx.screen.dpr = *dpr;
        // Push the new size into the `viewportSize$` source atom now
        // (event-driven), so subscribers re-layout in this same fixed-point
        // iteration. `sync_source` no-ops when the size is unchanged.
        cx.screen.sync_source(cx.boa);
        cx.element_tree.borrow_mut().mark_root_dirty();
        cx.request_paint();
    }
}
