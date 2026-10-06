//! Phase-5 flex-sizing fixtures — the operator audit's two evidence shapes.
//!
//! The audit flagged two playground cases as flex/Container sizing bugs:
//!
//! a) lazy-grid-gallery's header "renders as a ~48×36 box around 'Gallery'".
//!    The ACTUAL case source (verified before blaming the engine) authors the
//!    header as `Container().width_height(0.0, 40.0)` — and `box_size`'s
//!    documented builder idiom treats 0 as UNSET (see the `box_width` /
//!    `box_height` row comment: `width_height(220, 0)` = fixed width,
//!    unconstrained height). So the header is a shrink-wrap-width Container:
//!    Flutter lays that out child-sized, and the case gets a ~48×40 chip
//!    instead of the band the author colored. The plan's shape (a) is the
//!    FIXED-width twin: `width_height(400, 40)` as a Column child must lay
//!    out exactly 400×40 — pinned below as `fixed_size_column_child_honored`.
//!
//! b) table-reactive's "PLANET" header + bar sitting top-right of flush-left
//!    rows. Same root cause: `width_height(0.0, 32.0)` = unset width → the
//!    header shrink-wraps and the Column's default `CrossAlignment::Center`
//!    centers it over the full-width rows — visually "displaced". Zero-/
//!    unset-width children are LEGAL (the corpus uses `width_height(0.0, h)`
//!    by design); the engine contract pinned below is that they never corrupt
//!    SIBLING placement: the main-axis stack stays contiguous and every
//!    child lands inside the column's cross bounds.
//!
//! Verdicts (see the phase-5 commit body for the full table): both engine
//! shapes lay out Flutter-correct headlessly — the defect is case-authoring
//! (default cross alignment under a shrink-wrap header band). The corpus
//! pins at the bottom pin the REPAIRED cases: the gallery's flush-left
//! header block above its full-width grid (the boa restyle — the band
//! became a title), and table-reactive's header band flush-left above its
//! rows.

use tur_integration_tests::TurTestApp;

/// Absolute rect (x, y, w, h) of the keyed element, from the dev tool.
fn rect(app: &TurTestApp, key: &[&str]) -> (f64, f64, f64, f64) {
    let id = app.query_element(key).expect("element not found");
    let el = app.dev_tool_get_element(id).expect("dev tool element");
    (el.absolute.0, el.absolute.1, el.size.0, el.size.1)
}

// ── Shape (a): a fixed-size Container as a Column child ─────────────────

const FIXED_HEADER_RUT: &str = r#"
use tur::{ mount };
use tur_kit::{ Column, Container, Text };

entry fn start() -> u64 {
    // The lazy-grid-gallery header shape, with the size the audit assumed:
    // a 400×40 band above the rest of the column.
    let header = Container()
        .width_height(400.0, 40.0)
        .color(0x0F172AFFu64)
        .query_key("fs/header")
        .child(Text().text("Gallery").font_size(16.0).build())
        .build();
    let col = Column()
        .query_key("fs/col")
        .child(header)
        .build();
    mount(col);
    return 0;
}
"#;

#[test]
fn fixed_size_column_child_honored() {
    let mut app = TurTestApp::new(435.0, 400.0).unwrap();
    app.load_rut_module(FIXED_HEADER_RUT).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let (hx, hy, hw, hh) = rect(&app, &["fs", "header"]);
    let (cx, _, cw, _) = rect(&app, &["fs", "col"]);
    assert_eq!(
        (hw, hh),
        (400.0, 40.0),
        "width_height(400,40) must lay out 400×40, not shrink-wrap"
    );
    assert_eq!(hy, 0.0, "the band is the column's first child");
    assert_eq!(
        hx, cx,
        "the band is the column's widest child → flush with its leading edge"
    );
    assert_eq!(
        cx,
        (435.0 - cw) / 2.0,
        "the root centers the column in the viewport"
    );
}

// ── Shape (b): an unset-width (0 = unset idiom) child among siblings ────

const UNSET_WIDTH_HEADER_RUT: &str = r#"
use tur::{ mount };
use tur_kit::{ Column, Container, Text };

entry fn start() -> u64 {
    // The table-reactive shape verbatim: header `width_height(0, 32)` (0 =
    // unset per the box_size idiom) above a padded 150-wide row container.
    let head = Container()
        .width_height(0.0, 32.0)
        .color(0x1E293BFFu64)
        .query_key("fs/head")
        .child(Text().text("PLANET").font_size(12.0).build())
        .build();
    let row = Container()
        .width_height(150.0, 0.0)
        .padding(8.0)
        .query_key("fs/row0")
        .child(Text().text("Mercury").font_size(14.0).build())
        .build();
    let col = Column()
        .query_key("fs/col")
        .child(head)
        .child(row)
        .build();
    mount(col);
    return 0;
}
"#;

#[test]
fn unset_width_child_does_not_corrupt_sibling_placement() {
    let mut app = TurTestApp::new(435.0, 400.0).unwrap();
    app.load_rut_module(UNSET_WIDTH_HEADER_RUT).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let (hx, hy, hw, hh) = rect(&app, &["fs", "head"]);
    let (rx, ry, rw, rh) = rect(&app, &["fs", "row0"]);
    let (cx, _, cw, _) = rect(&app, &["fs", "col"]);

    // The unset-width header shrink-wraps to its label but keeps its fixed
    // height — it must stay INSIDE the column's cross bounds (never
    // displaced past the right edge, the audit's "top-right" symptom).
    // (Coordinates are absolute; the root centers the column, so the bounds
    // check is against the column's rect, not the viewport.)
    assert_eq!(hh, 32.0, "the header's fixed height is honored");
    assert!(
        hx >= cx && hx + hw <= cx + cw,
        "header must sit within the column's cross bounds \
         (header x={hx} w={hw}, column x={cx} w={cw})"
    );

    // Sibling placement is sane: the main-axis stack is contiguous — the
    // row starts exactly where the header ends — and stays flush at the
    // column's leading cross edge (it is the widest child).
    assert_eq!(hy, 0.0, "header is the first child");
    assert_eq!(
        ry, hh,
        "the row starts where the header ends — a shrink-wrap header \
         must not push or pull siblings"
    );
    assert_eq!(rx, cx, "the widest child sits at the column's leading edge");
    assert!(rw <= cw && ry + rh <= 400.0, "row stays inside the column");
}

// ── Corpus pins: the repaired evidence cases ────────────────────────────

/// lazy-grid-gallery (the boa restyle): the header is the flush-left title
/// Text now — it sits at the column's leading edge ABOVE the Expanded grid,
/// which starts below the header block (title + subtitle + chips intervene)
/// and fills the column's width. (The pre-restyle pin — a full-width 40px
/// band under `CROSS_ALIGN_STRETCH` — described a header design the boa
/// port replaced; the surviving intent is flush-left header, full-width
/// grid.)
#[test]
fn lazy_grid_gallery_header_sits_above_the_full_width_grid() {
    let mut app = TurTestApp::new(435.0, 600.0).unwrap();
    app.load_rut_bundle("lazy-grid-gallery").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let (hx, hy, _, hh) = rect(&app, &["gallery", "header"]);
    let (gx, gy, gw, _) = rect(&app, &["gallery", "grid"]);
    let (cx, _, cw, _) = rect(&app, &["gallery", "col"]);
    assert_eq!(hx, cx, "the title sits at the column's leading edge");
    assert_eq!(gx, cx, "the grid is flush with the title");
    assert!(
        gy >= hy + hh,
        "the grid starts below the header block \
         (title + subtitle + chip rows intervene)"
    );
    assert_eq!(gw, cw, "the Expanded grid fills the column's width");
}

/// table-reactive's header must sit left-aligned above its rows — the band's
/// leading edge matches the rows', and the main-axis stack stays contiguous.
#[test]
fn table_reactive_header_sits_left_aligned_above_rows() {
    let mut app = TurTestApp::new(435.0, 600.0).unwrap();
    app.load_rut_bundle("table-reactive").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // query_element is a pre-order DFS — the first match is item 0's row.
    let (hx, hy, hw, hh) = rect(&app, &["table", "head"]);
    let (rx, ry, _, _) = rect(&app, &["table", "row0"]);
    let (cx, cy, cw, _) = rect(&app, &["table", "col"]);
    assert_eq!(hy, cy, "the header is the column's first child");
    assert_eq!(hh, 32.0, "the header band keeps its 32px height");
    assert_eq!(hx, cx, "the header band is left-aligned with the rows");
    assert_eq!(
        hw, cw,
        "the header band spans the table's full width (Stretch)"
    );
    assert_eq!(
        ry,
        hy + hh,
        "the first row starts where the header band ends"
    );
    assert_eq!(rx, cx, "the rows sit flush left under the band");
}
