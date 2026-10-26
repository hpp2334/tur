//! Text glyphs must render UNHINTED (no vertical grid-fitting).
//!
//! glifo's glyph-run builder defaults to `hint: true` — vertical-only
//! smooth hinting, which grid-fits cap tops and baselines to whole pixel
//! boundaries. Flat-topped capitals then render with a hard-cut top edge:
//! a fully-saturated boundary row with literally zero ink above it (the
//! playground's "Designing" slice at dpr 100% — every 12px subtitle showed
//! it, unevenly by row). The renderer opts out (`.hint(false)`), so glyph
//! outlines rasterize with subpixel AA like Flutter's.
//!
//! The fixture renders a 12px "D" (bundled Roboto VF — the site's face,
//! pinned via `font_family("Roboto")`) with its baseline at exactly
//! 112.25 / 112.75 device px and asserts the AA-edge physics directly:
//!
//! - the first ink row must contain NO fully-saturated pixel (a
//!   grid-fitted edge produces one; a fractional cap top cannot);
//! - the 112.75 probe must carry an AA sliver in the row ABOVE its first
//!   ink row (the fractional edge straddles two rows — quantized
//!   placement leaves the row above pure background).
//!
//! Both assertions are font-metric-free: they hold for any face whose cap
//! top is fractional, and fail for the hinted renderer regardless of
//! where its grid-fitting lands.

use super::vello_app::TurVelloApp;
use parley::{FontContext, LayoutContext, StyleProperty};
use tur_engine::core::fonts::{
    FontLoader, fresh_font_context, load_font_stack, resolve_font_stack,
};

/// The bundled Roboto VF alone (the family the probe pins) — the mirror
/// layout computes the same baseline the engine does.
struct RobotoOnly;

impl FontLoader for RobotoOnly {
    fn load_preset_fonts(&self, fcx: &mut FontContext) {
        let roboto: &[u8] = include_bytes!("../../../tur-wasm/fonts/Roboto-VF.ttf");
        fcx.collection.register_fonts(roboto.to_vec().into(), None);
    }
}

/// The layout-local baseline of a single-line 12px "D" in Roboto — the
/// same value the engine's run encoder bakes into `TextGlyph.y`.
fn baseline_12px() -> f32 {
    let mut fcx = fresh_font_context();
    load_font_stack(&mut fcx, &RobotoOnly);
    let mut lcx = LayoutContext::<[u8; 4]>::new();
    let mut builder = lcx.ranged_builder(&mut fcx, "D", 1.0, false);
    builder.push_default(StyleProperty::FontSize(12.0));
    builder.push_default(StyleProperty::FontFamily(resolve_font_stack(
        Some("Roboto"),
        None,
    )));
    let mut layout = builder.build("D");
    layout.break_all_lines(None);
    layout
        .lines()
        .next()
        .expect("a single-line layout")
        .metrics()
        .baseline
}

/// Load the probe module: a 12px Roboto "D" whose baseline sits at exactly
/// `baseline_device` device px (the spacer height cancels the layout-local
/// baseline).
fn load_probe(app: &TurVelloApp, baseline_device: f64, baseline_local: f64) {
    let top = baseline_device - baseline_local;
    let source = format!(
        r#"
use tur_kit::handles::{{ mount }};
use tur_kit::layout::box::{{ SizedBox }};
use tur_kit::layout::flex::{{ Column }};
use tur_kit::reactive::{{ sv }};
use tur_kit::text::core::{{ Text }};

entry fn start() -> u64 {{
    mount(Column()
        .child(SizedBox(0.0, {top:.11}).build())
        .child(Text().text(sv("D")).font_size(12.0).font_family("Roboto").build())
        .build());
    return 0;
}}
"#
    );
    app.load_source(&source).unwrap();
}

/// The first row carrying ink (any channel below the near-white
/// threshold) inside the scan window.
fn ink_top(pixels: &[u8], phys_w: u32, x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> u32 {
    for py in y {
        for px in x.clone() {
            let idx = ((py * phys_w + px) * 4) as usize;
            if pixels[idx] < 250 {
                return py;
            }
        }
    }
    panic!("no ink found in the scan window");
}

/// The last row carrying ink inside the scan window.
fn ink_bottom(pixels: &[u8], phys_w: u32, x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> u32 {
    for py in y.rev() {
        for px in x.clone() {
            let idx = ((py * phys_w + px) * 4) as usize;
            if pixels[idx] < 250 {
                return py;
            }
        }
    }
    panic!("no ink found in the scan window");
}

/// Row `y`'s darkest pixel over the scan columns (255 = pure background).
fn row_min(pixels: &[u8], phys_w: u32, y: u32, x: std::ops::Range<u32>) -> u8 {
    let mut mn = 255u8;
    for px in x {
        let v = pixels[((y * phys_w + px) * 4) as usize];
        mn = mn.min(v);
    }
    mn
}

pub fn vello_text_renders_unhinted_glyphs() {
    let baseline_local = f64::from(baseline_12px());
    let app = TurVelloApp::new(120.0, 200.0, 1.0).unwrap();

    // Probe A — baseline 112.25 → snapped to 112: Roboto's cap top lands
    // at 103.47 (fractional), so the first ink row is 103 and MUST be
    // partial; the baseline edge sits exactly on the 112 boundary (crisp
    // full row — what snapping is for), so the last ink row is 111.
    load_probe(&app, 112.25, baseline_local);
    let px_a = app.render_to_pixels();
    let top_a = ink_top(&px_a, 120, 0..120, 60..170);
    assert_eq!(top_a, 103, "unhinted fractional cap top must ink row 103");
    let first_a = row_min(&px_a, 120, top_a, 0..120);
    assert!(
        first_a > 0,
        "the cap-top row must be anti-aliased — a fully-saturated pixel \
         means the edge was grid-fitted (vertical hinting is back)"
    );
    assert_eq!(
        ink_bottom(&px_a, 120, 0..120, 60..170),
        111,
        "the snapped baseline must end the glyph exactly at the 112 boundary"
    );

    // Probe B — baseline 112.75 → snapped to 113 (NEAREST, not the
    // upstream floor which would render it at 112): cap top 104.47 →
    // first ink row 104 (partial), last ink row 112.
    load_probe(&app, 112.75, baseline_local);
    let px_b = app.render_to_pixels();
    let top_b = ink_top(&px_b, 120, 0..120, 60..170);
    assert_eq!(top_b, 104, "baseline 112.75 must snap UP to 113 (nearest)");
    let first_b = row_min(&px_b, 120, top_b, 0..120);
    assert!(
        first_b > 0,
        "the cap-top row must be anti-aliased — a fully-saturated pixel \
         means the edge was grid-fitted (vertical hinting is back)"
    );
    assert_eq!(
        ink_bottom(&px_b, 120, 0..120, 60..170),
        112,
        "baseline 112.75 must draw at 113 (nearest), not floor to 112"
    );
}
