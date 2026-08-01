use super::vello_app::TurVelloApp;

pub fn vello_counter_app() {
    let app = TurVelloApp::new(1024.0, 768.0, 1.0).unwrap();
    app.load_bundle("vello-column-basic").unwrap();

    app.with_element_tree(|tree| {
        let root = tree.root_element().unwrap();
        assert!(!root.children.is_empty());
    });

    app.render();
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

fn test_dpr_render(dpr: f64) {
    let logical_w = 200u32;
    let logical_h = 200u32;
    let phys_w = (logical_w as f64 * dpr) as u32;
    let phys_h = (logical_h as f64 * dpr) as u32;

    let app = TurVelloApp::new(logical_w as f64, logical_h as f64, dpr).unwrap();
    app.load_bundle("four-color-quadrants").unwrap();
    app.render();

    let pixels = app.render_to_pixels();
    assert_eq!(
        pixels.len(),
        (phys_w * phys_h * 4) as usize,
        "dpr={dpr}: pixel buffer size mismatch"
    );

    let red = (255, 0, 0, 255);
    let green = (0, 255, 0, 255);
    let blue = (0, 0, 255, 255);
    let yellow = (255, 255, 0, 255);

    let sample_x = ((logical_w as f64 / 4.0) * dpr) as u32;
    let sample_y = ((logical_h as f64 / 4.0) * dpr) as u32;
    let cross_x = ((logical_w as f64 / 2.0 + logical_w as f64 / 4.0) * dpr) as u32;
    let cross_y = ((logical_h as f64 / 2.0 + logical_h as f64 / 4.0) * dpr) as u32;

    assert_color_approx(get_pixel(&pixels, phys_w, sample_x, sample_y), red, 5);
    assert_color_approx(get_pixel(&pixels, phys_w, cross_x, sample_y), green, 5);
    assert_color_approx(get_pixel(&pixels, phys_w, sample_x, cross_y), blue, 5);
    assert_color_approx(get_pixel(&pixels, phys_w, cross_x, cross_y), yellow, 5);
}

pub fn vello_dpr_1_renders_colors() {
    test_dpr_render(1.0);
}

pub fn vello_dpr_1_5_renders_colors() {
    test_dpr_render(1.5);
}

pub fn vello_dpr_2_renders_colors() {
    test_dpr_render(2.0);
}

pub fn vello_dpr_3_renders_colors() {
    test_dpr_render(3.0);
}

/// Verify that an `Image` element renders its uploaded pixels (the
/// `image-basic` case is a semi-transparent yellow 1x1 PNG stretched over the
/// canvas, which blends to pale yellow on the white background).
pub fn vello_image_renders() {
    let app = TurVelloApp::new(200.0, 100.0, 1.0).unwrap();
    app.load_bundle("image-basic").unwrap();
    app.render();

    let pixels = app.render_to_pixels();
    // Sample the centre pixel.
    let cx = 100u32;
    let cy = 50u32;
    let phys_w = 200u32;
    let (r, g, b, _a) = get_pixel(&pixels, phys_w, cx, cy);
    // The image is yellow (#ffff00) at ~50% alpha over a white background, so
    // the result must be pale yellow: red and green high, blue low. This proves
    // an image (not the white background) was drawn.
    assert!(
        r > 200 && g > 200 && b < 200,
        "image did not render: centre pixel was ({r},{g},{b}), expected pale yellow"
    );
}
