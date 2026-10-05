//! Phase-4 hit-test dead-zone fixtures — the operator audit's two shapes.
//!
//! The audit found two in-viewer controls that never fired on the rut build:
//! text-demo's "cycle maxLines" button (7 taps across its width, zero
//! response) and jigsaw-puzzle's piece (on_down/on_move wired, no drag ever
//! registered). Two suspected shapes:
//!
//! a) `PointerInteract` inside `ScrollView` content (the text-demo cycle
//!    button: a `MouseRegion`-wrapped pill at the bottom of a page taller
//!    than the viewport). Pinned unscrolled AND after a wheel scroll into
//!    view — the scrolled variant is the sharper suspect (stale content
//!    offsets in the hit walk would die exactly there).
//! b) `PointerInteract` inside `Positioned` inside `Stack` (the jigsaw
//!    piece). Pinned single-build (kit-law-correct, chained) AND the
//!    audited double-build (`.build()` → mutate → `.build()` → mount) to
//!    isolate whether the double-build alone explains a dead zone — it
//!    cannot: the kit's compile-time type check rejects the post-build
//!    mutation outright (see `double_build_is_rejected_at_compile_time`).
//!
//! Also pins the corpus jigsaw-puzzle case itself (its callbacks write into
//! atoms; the label must actually flip for the wiring to be observable).

use std::time::Duration;

use tur_integration_tests::TurTestApp;

fn center(app: &TurTestApp, key: &[&str]) -> (f64, f64) {
    let id = app.query_element(key).expect("element not found");
    let el = app
        .dev_tool_get_element(tur_engine::core::element::ElementNodeId::new(id.as_u64()).into())
        .expect("dev tool element");
    (
        el.absolute.0 + el.size.0 / 2.0,
        el.absolute.1 + el.size.1 / 2.0,
    )
}

fn label(app: &TurTestApp, key: &[&str]) -> String {
    let id = app.query_element(key).expect("label not found");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<tur_engine::builtin_plugins::text::TextElement>()
            .map(|c| {
                c.spans()
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
            })
    })
    .unwrap_or_default()
    .unwrap_or_default()
}

/// A mouse drag: down → moves → up with driven frames between (the mouse
/// path dispatches down immediately and moves while the composer tracks).
fn mouse_drag(app: &mut TurTestApp, start: (f64, f64), end: (f64, f64), steps: usize) {
    app.pointer_down(start.0, start.1);
    app.wait_for_timeout(Duration::ZERO);
    for i in 1..=steps {
        let frac = i as f64 / steps as f64;
        app.pointer_move(
            start.0 + (end.0 - start.0) * frac,
            start.1 + (end.1 - start.1) * frac,
        );
        app.wait_for_timeout(Duration::from_millis(16));
    }
    app.pointer_up(end.0, end.1);
    app.wait_for_timeout(Duration::from_millis(16));
}

// ── Shape (a): PointerInteract inside ScrollView content ────────────────

const SCROLL_BTN_RUT: &str = r#"
use tur::{ CURSOR_POINTER, CROSS_ALIGN_STRETCH, mount, rs_derive, rs_get_f64, rs_set_f64, rs_source_f64 };
use tur_kit::{ Column, Container, MouseRegion, PointerInteract, ScrollView, SizedBox, Text };

entry fn start() -> u64 {
    let taps = rs_source_f64();
    let label = rs_derive(fmt_taps, taps);
    // text-demo's cycle-button shape: MouseRegion(cursor) wrapping the
    // PointerInteract pill, at the bottom of a page taller than the
    // viewport, inside a ScrollView.
    let pill = Container()
        .padding(10.0)
        .radius(8.0)
        .color(0x4F46E5FFu64)
        .child(Text().text_bound_derived(label).font_size(13.0).query_key("dz/scroll-label").build())
        .build();
    let btn = MouseRegion()
        .cursor(CURSOR_POINTER)
        .child(
            PointerInteract()
                .id(taps)
                .on_tap(b_tap)
                .query_key("dz/scroll-btn")
                .child(pill)
                .build(),
        )
        .build();
    let page = Column()
        .cross_alignment(CROSS_ALIGN_STRETCH)
        .child(Text().text("top").build())
        .child(SizedBox(0.0, 900.0).build())
        .child(btn)
        .child(SizedBox(0.0, 40.0).build())
        .build();
    mount(ScrollView().child(page).build());
    return taps;
}

fn fmt_taps(v: f64) -> str {
    return f"taps {v as u64}";
}

fn b_tap(a: u64, _b: u64, _n: f64) {
    // The derive re-renders the label through fmt_taps.
    rs_set_f64(a, rs_get_f64(a) + 1.0);
}
"#;

fn scroll_btn_app() -> TurTestApp {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(SCROLL_BTN_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    app
}

#[test]
fn scroll_view_button_taps_at_laid_out_position() {
    let mut app = scroll_btn_app();
    assert_eq!(label(&app, &["dz", "scroll-label"]), "taps 0");

    // Unscrolled, the button sits below the fold — bring it into view the
    // way the operator does (wheel down over the scroller), then tap at its
    // laid-out on-screen position. Delta 1000 over-scrolls; the engine
    // clamps to max_scroll_extent (content ~1000 − viewport 300).
    app.wheel(0.0, 1000.0, 200.0, 150.0);
    app.wait_for_timeout(Duration::ZERO);
    let (cx, cy) = center(&app, &["dz", "scroll-btn"]);
    assert!(
        cy < 300.0,
        "the wheel should have brought the button into the viewport, cy={cy}"
    );
    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);

    assert_eq!(
        label(&app, &["dz", "scroll-label"]),
        "taps 1",
        "tap inside scrolled ScrollView content must reach the button"
    );
}

#[test]
fn scroll_view_button_taps_unscrolled_when_in_view() {
    // Same shape with a short page — the button in view at offset 0. Isolates
    // "in-scroll taps work at all" from the scrolled-position case above.
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(SCROLL_BTN_RUT.replace("900.0", "20.0").as_str())
        .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let (cx, cy) = center(&app, &["dz", "scroll-btn"]);
    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);

    assert_eq!(
        label(&app, &["dz", "scroll-label"]),
        "taps 1",
        "tap inside unscrolled ScrollView content must reach the button"
    );
}

// ── Shape (b): PointerInteract inside Positioned inside Stack ───────────

const STACK_PIECE_SINGLE_BUILD_RUT: &str = r#"
use tur::{ ALIGN_TOP_LEFT, mount, rs_set_str, rs_source_str };
use tur_kit::{ Container, PointerInteract, Positioned, SizedBox, Stack, Text };

entry fn start() -> u64 {
    let piece = rs_source_str("idle");
    let b = Container()
        .width_height(80.0, 80.0)
        .color(0x6366F1FFu64)
        .query_key("dz/piece")
        .child(Text().text_bound(piece).query_key("dz/piece-label").build())
        .build();
    let pad = PointerInteract()
        .id(piece)
        .on_down(g_down)
        .on_move(g_move)
        .on_up(g_up)
        .query_key("dz/piece-pad")
        .child(b)
        .build();
    mount(Stack()
        .alignment(ALIGN_TOP_LEFT)
        .child(SizedBox(300.0, 300.0).child(Container().build()).build())
        .child(Positioned().left(10.0).top(10.0).child(pad).build())
        .build());
    return piece;
}

fn g_down(piece: u64, _lx: f64, _ly: f64, _gx: f64, _gy: f64, _btn: u64) {
    rs_set_str(piece, "down");
}

fn g_move(piece: u64, _lx: f64, _ly: f64, _gx: f64, _gy: f64, _btn: u64) {
    rs_set_str(piece, "moving");
}

fn g_up(piece: u64, _lx: f64, _ly: f64, _gx: f64, _gy: f64, _btn: u64) {
    rs_set_str(piece, "up");
}
"#;

// The audited double-build shape: `stack = Stack()…build()` (a View), then
// `.child(pos)` on that View, then `mount(stack.build())`. See the test
// below for the verdict this shape exists to isolate.
const STACK_PIECE_DOUBLE_BUILD_RUT: &str = r#"
use tur::{ ALIGN_TOP_LEFT, mount, rs_set_str, rs_source_str };
use tur_kit::{ Container, PointerInteract, Positioned, SizedBox, Stack, Text };

entry fn start() -> u64 {
    let piece = rs_source_str("idle");
    let b = Container()
        .width_height(80.0, 80.0)
        .color(0x6366F1FFu64)
        .query_key("dz/piece")
        .child(Text().text_bound(piece).query_key("dz/piece-label").build())
        .build();
    let pad = PointerInteract()
        .id(piece)
        .on_down(g_down)
        .on_move(g_move)
        .on_up(g_up)
        .query_key("dz/piece-pad")
        .child(b)
        .build();
    let mut stack = Stack()
        .alignment(ALIGN_TOP_LEFT)
        .child(SizedBox(300.0, 300.0).child(Container().build()).build())
        .build();
    stack.child(Positioned().left(10.0).top(10.0).child(pad).build());
    mount(stack.build());
    return piece;
}

fn g_down(piece: u64, _lx: f64, _ly: f64, _gx: f64, _gy: f64, _btn: u64) {
    rs_set_str(piece, "down");
}

fn g_move(piece: u64, _lx: f64, _ly: f64, _gx: f64, _gy: f64, _btn: u64) {
    rs_set_str(piece, "moving");
}

fn g_up(piece: u64, _lx: f64, _ly: f64, _gx: f64, _gy: f64, _btn: u64) {
    rs_set_str(piece, "up");
}
"#;

fn stack_piece_app(source: &str) -> TurTestApp {
    let mut app = TurTestApp::new(300.0, 400.0).unwrap();
    app.load_rut_module(source).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    app
}

fn assert_stack_piece_drag(app: &mut TurTestApp, what: &str) {
    assert_eq!(label(app, &["dz", "piece-label"]), "idle", "{what}: seed");

    // The piece is a Positioned(left 10, top 10) 80×80 box over a 300×300
    // slab. Drag from its center — on_down fires on the down, on_move rides
    // the drag, and g_up's restore is the final state.
    let (cx, cy) = center(app, &["dz", "piece"]);
    mouse_drag(app, (cx, cy), (cx + 40.0, cy + 40.0), 4);

    assert_eq!(
        label(app, &["dz", "piece-label"]),
        "up",
        "{what}: drag must dispatch down+move+up (g_up's restore is the \
         observable final state)"
    );
}

#[test]
fn positioned_in_stack_piece_receives_drag_single_build() {
    let mut app = stack_piece_app(STACK_PIECE_SINGLE_BUILD_RUT);
    assert_stack_piece_drag(&mut app, "single-build");
}

// The double-build isolation experiment: `stack = Stack()…build()` (so
// `stack` holds a VIEW, not the builder), then `.child(pos)` on that View,
// then `mount(stack.build())`. VERDICT: the shape cannot exist — the kit's
// compile-time type check rejects the post-build mutation outright
// ("`opaque` has no methods": `build()` returns `View` = `opaque`, and
// opaques carry no builder methods), so the audited malformation fails the
// module at LOAD, never mounting a dead tree. Combined with the green
// single-build variant above, the Positioned-in-Stack dead zone cannot be
// an engine hit-test bug — it is case-authoring (or browser-side), and the
// kit law ("never build-then-mutate") is mechanically enforced at the kit
// boundary.
#[test]
fn double_build_is_rejected_at_compile_time() {
    let app = TurTestApp::new(300.0, 400.0).unwrap();
    let err = app
        .load_rut_module(STACK_PIECE_DOUBLE_BUILD_RUT)
        .expect_err("build-then-mutate must fail at COMPILE time");
    let msg = format!("{err}");
    assert!(
        msg.contains("opaque"),
        "the diagnostic should name the opaque-method misuse, got: {msg}"
    );
}

// ── The corpus jigsaw-puzzle case itself ────────────────────────────────

#[test]
fn jigsaw_corpus_piece_receives_drag() {
    let mut app = TurTestApp::new(300.0, 400.0).unwrap();
    app.load_rut_bundle("jigsaw-puzzle").unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(label(&app, &["piece", "label"]), "A", "seed label");

    // Drag down + moves, then hold (no up): on_down must have flipped the
    // bound label to "dragging" — the audit's dead-zone probe, now
    // observable because the label is BOUND to the piece atom.
    let (cx, cy) = center(&app, &["rut", "gesture"]);
    app.pointer_down(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
    app.pointer_move(cx + 13.0, cy + 13.0);
    app.wait_for_timeout(Duration::from_millis(16));
    app.pointer_move(cx + 27.0, cy + 27.0);
    app.wait_for_timeout(Duration::from_millis(16));
    assert_eq!(
        label(&app, &["piece", "label"]),
        "dragging",
        "mid-drag: on_down + on_move must have fired"
    );

    // Release: g_up restores the piece label.
    app.pointer_up(cx + 40.0, cy + 40.0);
    app.wait_for_timeout(Duration::from_millis(16));
    assert_eq!(
        label(&app, &["piece", "label"]),
        "A",
        "on_up restores the label"
    );
}
