//! Deferred-resize tests: a resize must not disturb the presented surface
//! until the replacement frame renders. The worker stamps the viewport a
//! batch was laid out for onto `HostMsg::RenderCommands`, and the host
//! syncs its renderer to it at the **render commit point** — immediately
//! before playing that batch back (`HostBackend::render_batch`), deduped
//! per actual change. Geometry and content therefore land in one atomic
//! operation: the backing-store swap (browser canvas `width`/`height`, wgpu
//! swapchain reconfigure) destroys the currently-presented pixels, so
//! doing it at resize-event-receipt time leaves a visible cleared/blank
//! gap for the whole worker round-trip + vsync until the new frame lands
//! (the resize white flash).

use std::time::Duration;

use crate::vello_app::TurVelloApp;

/// A resize records nothing on the host renderer: `TurApp::resize` only
/// forwards the shell event to the worker, so with no frame rendered since
/// the resize the renderer still presents the LAST frame at the OLD size
/// (the browser stretches it to the new CSS box — blurry beats a white
/// flash). The first frame rendered after the resize carries the new
/// viewport and the host applies it atomically with that frame's content.
pub fn resize_defers_surface_swap_until_next_frame() {
    let app = TurVelloApp::new(64.0, 48.0, 1.0).expect("harness build");
    // Load content that actually PAINTS (colored quadrants). An empty
    // paint (e.g. colorless containers that fit) is an empty batch, which
    // the loop legitimately skips — no frame, no swap.
    app.load_bundle("four-color-quadrants")
        .expect("bundle load");
    // Settle the bootstrap frame at 64x48.
    app.wait_for_timeout(Duration::ZERO);

    // Resize with NO frame rendered afterwards.
    app.app().resize(128, 96, 1.0);

    // The presented surface geometry must still be the OLD size — an
    // immediate surface swap here is the white-flash bug (the cleared
    // backing store is composited for the entire worker round-trip until
    // the new frame arrives).
    let pixels = app.render_to_pixels();
    assert_eq!(
        pixels.len(),
        64 * 48 * 4,
        "resize must not disturb the presented surface before the next frame renders"
    );

    // Drive the frame laid out at the new size: its batch carries the new
    // viewport, applied atomically with it at the render commit point.
    app.wait_for_timeout(Duration::ZERO);
    let pixels = app.render_to_pixels();
    assert_eq!(
        pixels.len(),
        128 * 96 * 4,
        "the first frame after a resize applies the new surface size"
    );
}
