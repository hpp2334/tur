use crate::core::platform::{PlatformEvent, ShellEventPayload};
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

pub struct ResizeSubsystem;

impl Subsystem for ResizeSubsystem {
    fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent) {
        let ShellEventPayload::Resize {
            logical_width,
            logical_height,
            dpr,
        } = event.payload()
        else {
            return;
        };
        let root = event.view_root_id();

        // The render target lives on main; the embedder resizes it directly
        // at event-receipt time via `TurApp::resize_root` (which also
        // forwards this resize shell event to the worker). Here we only
        // update the worker-side per-root screen state.
        let tree = {
            let mut roots = cx.view_roots.borrow_mut();
            let Some(slot) = roots.get_mut(root) else {
                return;
            };
            slot.screen.logical_size = (*logical_width as f64, *logical_height as f64);
            slot.screen.dpr = *dpr;
            // Push the new size into the root's `viewportSize$` source atom
            // now (event-driven), so subscribers re-layout in this same
            // fixed-point iteration. `sync_source` no-ops when unchanged.
            slot.screen.sync_source(cx.boa);
            slot.tree.clone()
        };
        // Only the resized root's subtree re-lays-out — one tree per view
        // root, so marking the whole tree dirty marks exactly that root's
        // subtree. Other roots' trees are untouched.
        tree.mark_root_dirty();
        cx.request_paint();
    }
}
