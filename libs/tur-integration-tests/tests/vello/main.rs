mod raw;
mod snapshot;
mod surface_lifecycle;
mod vello_app;

use libtest_mimic::{Arguments, Trial};

fn main() {
    let mut args = Arguments::from_args();
    // Each trial creates a real window + GPU surface (minifb + wgpu), which
    // macOS only allows on the process main thread. Default to 1 thread —
    // with cargo-nextest each test already runs in its own process, so
    // sequential-in-process costs nothing (an explicit --test-threads still
    // wins).
    if args.test_threads.is_none() {
        args.test_threads = Some(1);
    }
    let tests = vec![
        Trial::test("vello_counter_app", || {
            raw::vello_counter_app();
            Ok(())
        }),
        Trial::test("vello_dpr_1_renders_colors", || {
            raw::vello_dpr_1_renders_colors();
            Ok(())
        }),
        Trial::test("vello_dpr_1_5_renders_colors", || {
            raw::vello_dpr_1_5_renders_colors();
            Ok(())
        }),
        Trial::test("vello_dpr_2_renders_colors", || {
            raw::vello_dpr_2_renders_colors();
            Ok(())
        }),
        Trial::test("vello_dpr_3_renders_colors", || {
            raw::vello_dpr_3_renders_colors();
            Ok(())
        }),
        Trial::test("vello_image_renders", || {
            raw::vello_image_renders();
            Ok(())
        }),
        Trial::test("vello_snapshot_reported_stretch_bug", || {
            snapshot::reported_stretch_bug_renders_red_strip();
            Ok(())
        }),
        Trial::test("vello_container_clip_rounded", || {
            snapshot::container_clip_rounded_corner_clipped();
            Ok(())
        }),
        Trial::test("vello_base_color_paints_configured_background", || {
            snapshot::base_color_paints_configured_background();
            Ok(())
        }),
        Trial::test("vello_base_color_supports_alpha", || {
            snapshot::base_color_supports_alpha();
            Ok(())
        }),
        Trial::test("vello_init_surface_zero_area_degrades", || {
            surface_lifecycle::init_surface_zero_area_degrades();
            Ok(())
        }),
        Trial::test("vello_resize_zero_area_degrades_and_recovers", || {
            surface_lifecycle::resize_zero_area_degrades_and_recovers();
            Ok(())
        }),
    ];
    libtest_mimic::run(&args, tests).exit();
}
