use std::path::PathBuf;

use image::ExtendedColorType;
use tur_engine::core::render::brush::Color;

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
    (
        pixels[idx],
        pixels[idx + 1],
        pixels[idx + 2],
        pixels[idx + 3],
    )
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
    app.wait_for_timeout(std::time::Duration::ZERO);

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

/// `Container({ borderRadius: 40, clipBehavior: ClipBehavior.HardEdge })`
/// must clip its child subtree to the rounded decoration shape (Flutter
/// parity). The child is a solid red 200x200 square that fills the
/// container; without clipping its square corner would paint red right up
/// to (0,0). With clipping, the area outside the 40px rounded arc is left
/// unpainted (page background = white), while the center stays red (the
/// child still paints inside the rounded shape).
pub fn container_clip_rounded_corner_clipped() {
    let logical_w = 200u32;
    let logical_h = 200u32;

    let app = TurVelloApp::new(logical_w as f64, logical_h as f64, 1.0).unwrap();
    app.load_bundle("container-clip-rounded").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let pixels = app.render_to_pixels();
    assert_eq!(
        pixels.len(),
        (logical_w * logical_h * 4) as usize,
        "pixel buffer size mismatch"
    );

    // Corner (3,3) is well outside the 40px rounded arc — its distance from
    // the arc center at (40,40) is ~52 > 40 — so the red child must have
    // been clipped away, leaving the unpainted page background (white).
    let corner = get_pixel(&pixels, logical_w, 3, 3);
    assert_color_approx(corner, (255, 255, 255, 255), 8);

    // Center is inside the rounded rect — the red child still paints there.
    let center = get_pixel(&pixels, logical_w, 100, 100);
    assert_color_approx(center, (255, 0, 0, 255), 8);

    dump_snapshot_if_requested("container-clip-rounded", &pixels, logical_w, logical_h);
}

/// `VelloRenderer::base_color` — the configured base color replaces the
/// default opaque-white page background: unpainted areas render in the base
/// color, content still paints over it.
pub fn base_color_paints_configured_background() {
    let logical_w = 400u32;
    let logical_h = 200u32;
    let base = Color::rgb(30, 40, 50);

    let app =
        TurVelloApp::new_with_base_color(logical_w as f64, logical_h as f64, 1.0, base).unwrap();
    app.load_bundle("column-stretch-container-text").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let pixels = app.render_to_pixels();
    assert_eq!(
        pixels.len(),
        (logical_w * logical_h * 4) as usize,
        "pixel buffer size mismatch"
    );

    // Below the red strip: the configured base color (the same sample point
    // `reported_stretch_bug_renders_red_strip` asserts is white under the
    // default config).
    let below = get_pixel(&pixels, logical_w, 200, 150);
    assert_color_approx(below, (30, 40, 50, 255), 8);

    // Content still paints over the base: the red strip stays red.
    let strip = get_pixel(&pixels, logical_w, 200, 45);
    assert_color_approx(strip, (255, 0, 0, 255), 8);

    dump_snapshot_if_requested("base-color-opaque", &pixels, logical_w, logical_h);
}

/// `VelloRenderer::base_color` honors alpha: a translucent base composites
/// over vello-hybrid's transparent clear (the frame's unpainted areas keep
/// the base color's alpha).
pub fn base_color_supports_alpha() {
    let logical_w = 400u32;
    let logical_h = 200u32;
    let base = Color::rgba(255, 0, 0, 128);

    let app =
        TurVelloApp::new_with_base_color(logical_w as f64, logical_h as f64, 1.0, base).unwrap();
    app.load_bundle("column-stretch-container-text").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let pixels = app.render_to_pixels();
    assert_eq!(
        pixels.len(),
        (logical_w * logical_h * 4) as usize,
        "pixel buffer size mismatch"
    );

    // Below the strip: the base color's alpha survives compositing. The
    // strip renderer writes premultiplied output, so each channel reads
    // back at value × alpha/255 (red: 255 × 128/255 = 128).
    let below = get_pixel(&pixels, logical_w, 200, 150);
    assert_color_approx(below, (128, 0, 0, 128), 8);

    // The red content strip stays fully opaque red.
    let strip = get_pixel(&pixels, logical_w, 200, 45);
    assert_color_approx(strip, (255, 0, 0, 255), 8);

    dump_snapshot_if_requested("base-color-alpha", &pixels, logical_w, logical_h);
}
