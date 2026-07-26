use std::path::PathBuf;

use image::ExtendedColorType;

use super::vello_app::TurVelloApp;

/// Where to dump PNG snapshots when `TUR_DUMP_SNAPSHOTS=1` is set. Defaults to
/// the opencode tmp dir, falling back to the system temp dir.
fn snapshot_dir() -> PathBuf {
    let base = PathBuf::from("/var/folders/w9/9nyd_g1x2_55pvgtzpz77z9h0000gn/T/opencode");
    if base.is_dir() {
        base
    } else {
        std::env::temp_dir()
    }
}

fn dump_snapshot_if_requested(name: &str, pixels: &[u8], w: u32, h: u32) {
    if std::env::var("TUR_DUMP_SNAPSHOTS").as_deref() != Ok("1") {
        return;
    }
    let path = snapshot_dir().join(format!("{name}.png"));
    if let Err(e) = image::save_buffer(&path, pixels, w, h, ExtendedColorType::Rgba8) {
        eprintln!("[snapshot] failed to write {path:?}: {e}");
    } else {
        println!("[snapshot] {name} -> {} ({w}x{h})", path.display());
    }
}

fn get_pixel(pixels: &[u8], phys_w: u32, px: u32, py: u32) -> (u8, u8, u8, u8) {
    let idx = ((py * phys_w + px) * 4) as usize;
    (pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3])
}

fn assert_color_approx(actual: (u8, u8, u8, u8), expected: (u8, u8, u8, u8), tolerance: u8) {
    let (ar, ag, ab, aa) = actual;
    let (er, eg, eb, ea) = expected;
    assert!(
        ar.abs_diff(er) <= tolerance
            && ag.abs_diff(eg) <= tolerance
            && ab.abs_diff(eb) <= tolerance
            && aa.abs_diff(ea) <= tolerance,
        "color mismatch: got ({ar},{ag},{ab},{aa}), expected ({er},{eg},{eb},{ea}) ±{tolerance}"
    );
}

/// Reported minimal repro, verified at the pixel level:
/// Column(crossAlignment: Stretch) > Container(color: red, padding: 20) >
/// Text(white). The layout geometry tests (tests/layout) already prove the
/// Container is 400x54 and the text is positioned at (20, 20). This test
/// additionally proves the vello/wgpu paint path actually paints the red
/// strip — i.e. the reported "nothing renders on Android" symptom does NOT
/// reproduce here.
pub fn reported_stretch_bug_renders_red_strip() {
    let logical_w = 400u32;
    let logical_h = 200u32;

    let app = TurVelloApp::new(logical_w as f64, logical_h as f64, 1.0).unwrap();
    app.load_bundle("column-stretch-container-text").unwrap();
    app.render();

    let pixels = app.render_to_pixels();
    assert_eq!(
        pixels.len(),
        (logical_w * logical_h * 4) as usize,
        "pixel buffer size mismatch"
    );

    // The red container is full-width (400) at the top, ~54px tall (text 14 +
    // padding 20*2). Sample the middle of the strip, below the text baseline
    // to avoid hitting a glyph: x=200 (center, away from the left-aligned
    // text), y=45 (inside the strip, past the text).
    let strip = get_pixel(&pixels, logical_w, 200, 45);
    assert_color_approx(strip, (255, 0, 0, 255), 8);

    // Below the strip the background should be white (the default canvas).
    let below = get_pixel(&pixels, logical_w, 200, 150);
    assert_color_approx(below, (255, 255, 255, 255), 8);

    dump_snapshot_if_requested("reported-stretch-bug", &pixels, logical_w, logical_h);
}
