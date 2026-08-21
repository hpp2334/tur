use crate::core::platform::PlatformEvent;
use crate::core::shell::ShellEvent;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

pub struct ResizeSubsystem;

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
