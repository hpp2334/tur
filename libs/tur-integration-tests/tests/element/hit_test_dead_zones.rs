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
//! Also pins the corpus jigsaw-puzzle case — since Phase 9 the full 3×3
//! game (drag → snap → count → shuffle → solve), riding the same
//! Positioned-in-Stack shape the verdict cleared.

use std::time::Duration;

use tur_engine::core::element::ElementKind;
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

use tur_kit::flags::{ Align, CrossAlign, Cursor };
use tur_kit::gesture::pointer::{ MouseRegion, PointerInteract };
use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ Container, SizedBox };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ DeriveCtx, Mutation, MutationCtx, Readable, Source, derive, mutate, source };
use tur_kit::scroll::{ ScrollView };
use tur_kit::text::core::{ Text };

entry fn start() -> u64 {
    let taps: Readable<f64> = source<f64>(0.0);
    let label: Readable<str> = derive<str>(fn (ctx: DeriveCtx) -> str {
        return fmt_taps(ctx.get<f64>(taps));
    });
    // text-demo's cycle-button shape: MouseRegion(cursor) wrapping the
    // PointerInteract pill, at the bottom of a page taller than the
    // viewport, inside a ScrollView.
    let pill = Container()
        .padding(10.0)
        .radius(8.0)
        .color(0x4F46E5FFu64)
        .child(Text().text_bound(label).font_size(13.0).query_key("dz/scroll-label").build())
        .build();
    let b_tap = mutate(fn (ctx: MutationCtx, _e: nil) {
        // The derive re-renders the label through fmt_taps.
        ctx.set<f64>(taps, ctx.get<f64>(taps) + 1.0);
    });
    let btn = MouseRegion()
        .cursor(Cursor.Pointer)
        .child(
            PointerInteract()
                .on_click(b_tap)
                .query_key("dz/scroll-btn")
                .child(pill)
                .build(),
        )
        .build();
    let page = Column()
        .cross_alignment(CrossAlign.Stretch)
        .child(Text().text("top").build())
        .child(SizedBox(0.0, 900.0).build())
        .child(btn)
        .child(SizedBox(0.0, 40.0).build())
        .build();
    mount(ScrollView().child(page).build());
    return taps.atom_id();
}

fn fmt_taps(v: f64) -> str {
    return f"taps {v as u64}";
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

use tur_kit::flags::{ Align, CrossAlign, Cursor };
use tur_kit::gesture::pointer::{ PointerEvent, PointerInteract };
use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ Container, SizedBox };
use tur_kit::layout::stack::{ Positioned, Stack };
use tur_kit::reactive::{ DeriveCtx, Mutation, MutationCtx, Readable, Source, derive, mutate, source };
use tur_kit::text::core::{ Text };

entry fn start() -> u64 {
    let piece: Readable<str> = source<str>("idle");
    let b = Container()
        .width_height(80.0, 80.0)
        .color(0x6366F1FFu64)
        .query_key("dz/piece")
        .child(Text().text_bound(piece).query_key("dz/piece-label").build())
        .build();
    let b_down = mutate<PointerEvent>(fn (ctx: MutationCtx, _ev: PointerEvent) {
        ctx.set<str>(piece, "down");
    });
    let b_move = mutate<PointerEvent>(fn (ctx: MutationCtx, _ev: PointerEvent) {
        ctx.set<str>(piece, "moving");
    });
    let b_up = mutate<PointerEvent>(fn (ctx: MutationCtx, _ev: PointerEvent) {
        ctx.set<str>(piece, "up");
    });
    let pad = PointerInteract()
        .on_pointer_down(b_down)
        .on_pointer_move(b_move)
        .on_pointer_up(b_up)
        .query_key("dz/piece-pad")
        .child(b)
        .build();
    mount(Stack()
        .alignment(Align.TopLeft)
        .child(SizedBox(300.0, 300.0).child(Container().build()).build())
        .child(Positioned().left(10.0).top(10.0).child(pad).build())
        .build());
    return piece.atom_id();
}
"#;

// The audited double-build shape: `stack = Stack()…build()` (a View), then
// `.child(pos)` on that View, then `mount(stack.build())`. See the test
// below for the verdict this shape exists to isolate.
const STACK_PIECE_DOUBLE_BUILD_RUT: &str = r#"

use tur_kit::flags::{ Align, CrossAlign, Cursor };
use tur_kit::gesture::pointer::{ PointerEvent, PointerInteract };
use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ Container, SizedBox };
use tur_kit::layout::stack::{ Positioned, Stack };
use tur_kit::reactive::{ DeriveCtx, Mutation, MutationCtx, Readable, Source, derive, mutate, source };
use tur_kit::text::core::{ Text };

entry fn start() -> u64 {
    let piece: Readable<str> = source<str>("idle");
    let b = Container()
        .width_height(80.0, 80.0)
        .color(0x6366F1FFu64)
        .query_key("dz/piece")
        .child(Text().text_bound(piece).query_key("dz/piece-label").build())
        .build();
    let b_down = mutate<PointerEvent>(fn (ctx: MutationCtx, _ev: PointerEvent) {
        ctx.set<str>(piece, "down");
    });
    let b_move = mutate<PointerEvent>(fn (ctx: MutationCtx, _ev: PointerEvent) {
        ctx.set<str>(piece, "moving");
    });
    let b_up = mutate<PointerEvent>(fn (ctx: MutationCtx, _ev: PointerEvent) {
        ctx.set<str>(piece, "up");
    });
    let pad = PointerInteract()
        .on_pointer_down(b_down)
        .on_pointer_move(b_move)
        .on_pointer_up(b_up)
        .query_key("dz/piece-pad")
        .child(b)
        .build();
    let mut stack = Stack()
        .alignment(Align.TopLeft)
        .child(SizedBox(300.0, 300.0).child(Container().build()).build())
        .build();
    stack.child(Positioned().left(10.0).top(10.0).child(pad).build());
    mount(stack.build());
    return piece.atom_id();
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
// ("`View` has no method `child`": `build()` returns the `View` newtype,
// which carries no builder methods), so the audited malformation fails the
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
        msg.contains("View"),
        "the diagnostic should name the post-build method misuse, got: {msg}"
    );
}

// ── The corpus jigsaw-puzzle case (the Phase-9 full game) ────────────────
//
// The Phase-9 rewrite turned the fixture case into the full 3×3 game:
// bound `left`/`top` atoms per piece (the drag rail), `.ids(i, i)` shared
// callbacks, the "N / 9 placed" derive badge, Shuffle (a stash-cell
// Fisher-Yates re-deal), and the Solved! overlay. The round-5 parity pass
// replaced the invented cyan snap-highlight with the boa game feel: a
// per-piece STATE SHADOW through the bound shadow rows (loose black soft
// → dragging hard → placed own-hue glow) and the 180ms lift (a shared
// easeOut controller over a per-piece bound scale). This pins the whole
// loop headlessly — the drag rail (down → move → up on a
// Positioned-in-Stack piece), the shadow/lift state changes, snap +
// non-snap, the placed lock, the re-deal, and the solve.

fn jigsaw_app() -> TurTestApp {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_bundle("jigsaw-puzzle").unwrap();
    app.wait_for_timeout(Duration::ZERO);
    app
}

// A piece's target slot: its bound label names it, 1-based.
fn jigsaw_target(app: &TurTestApp, piece: u64) -> u64 {
    let key = format!("piece-{piece}-label");
    label(app, &["jw", &key])
        .trim()
        .parse::<u64>()
        .expect("numeric label")
        - 1
}

// The piece face's painted (shadow_color, shadow_blur, shadow_dy) — the
// state-shadow rails.
fn jigsaw_shadow(
    app: &TurTestApp,
    piece: u64,
) -> (
    Option<tur_engine::core::render::brush::Color>,
    Option<f64>,
    Option<f64>,
) {
    let id = tur_engine::core::element::ElementNodeId::new(
        app.query_element(&["jw", &format!("piece-{piece}")])
            .unwrap()
            .as_u64(),
    );
    app.with_element(id, |e| {
        let c = e
            .cast::<tur_engine::builtin_plugins::layout::ContainerElement>()
            .unwrap();
        (
            c.painted_shadow_color(),
            c.painted_shadow_blur(),
            c.painted_shadow_dy(),
        )
    })
    .unwrap()
}

// The piece lift scale — the Transform wrapping the pad (the pad's parent).
fn jigsaw_lift(app: &TurTestApp, piece: u64) -> f64 {
    let pad = tur_engine::core::element::ElementNodeId::new(
        app.query_element(&["jw", &format!("pad-{piece}")])
            .unwrap()
            .as_u64(),
    );
    let xf = {
        let tree = app.element_tree();
        tree.get_element(pad)
            .and_then(|n| n.parent)
            .map(|p| tur_engine::core::element::ElementNodeId::new(p.as_u64()))
            .expect("the pad's lift Transform")
    };
    app.with_element(xf, |e| {
        e.cast::<tur_engine::builtin_plugins::effects::TransformElement>()
            .map(|t| t.painted_scale())
    })
    .unwrap()
    .unwrap()
}

/// The boa `dragScale$` semantics: the lift eases up on grab and STAYS at
/// LIFT_MAX for the whole grab — only the RELEASE (the controller's
/// reverse) settles it back. A long hold must not sink the piece back to
/// rest scale mid-drag.
#[test]
fn jigsaw_lift_stays_up_while_held() {
    let mut app = jigsaw_app();
    let from = center(&app, &["jw", "piece-0"]);
    app.pointer_down(from.0, from.1);
    app.wait_for_timeout(Duration::from_millis(16));
    app.pointer_move(from.0 + 8.0, from.1 + 8.0);
    app.wait_for_timeout(Duration::ZERO);
    // Hold well past the 180ms forward ease.
    app.wait_for_timeout(Duration::from_millis(400));
    let held = jigsaw_lift(&app, 0);
    assert!(
        (held - 1.1).abs() < 0.02,
        "the lift stays at LIFT_MAX while the piece is held (boa dragScale$); got {held}"
    );
    app.pointer_up(from.0 + 8.0, from.1 + 8.0);
    app.wait_for_timeout(Duration::from_millis(300));
    let released = jigsaw_lift(&app, 0);
    assert!(
        (released - 1.0).abs() < 0.02,
        "the release settles the lift back to 1; got {released}"
    );
}

#[test]
fn jigsaw_game_drags_snaps_counts_and_solves() {
    let mut app = jigsaw_app();

    // Boot: counter seeds at 0, 9 pieces + 9 ghosts, piece 0 at rest — the
    // loose soft shadow, scale 1.
    assert_eq!(
        label(&app, &["jw", "counter"]),
        "0 / 9 placed",
        "seed count"
    );
    let (sc, sb, sd) = jigsaw_shadow(&app, 0);
    assert_eq!(
        sc,
        Some(tur_engine::core::render::brush::Color::rgba(0, 0, 0, 0x6E)),
        "loose shadow color"
    );
    assert_eq!(sb, Some(10.0), "loose shadow blur");
    assert_eq!(sd, Some(4.0), "loose shadow dy");
    assert!(
        (jigsaw_lift(&app, 0) - 1.0).abs() < 0.001,
        "at rest scale 1"
    );

    // WRONG slot: the piece follows the drag, nothing snaps, nothing counts.
    let target0 = jigsaw_target(&app, 0);
    let wrong = (target0 + 1) % 9;
    let from = center(&app, &["jw", "piece-0"]);
    let wrong_center = center(&app, &["jw", &format!("ghost-{wrong}")]);
    mouse_drag(&mut app, from, wrong_center, 6);
    assert_eq!(
        label(&app, &["jw", "counter"]),
        "0 / 9 placed",
        "wrong slot must not count"
    );
    let dropped = center(&app, &["jw", "piece-0"]);
    assert!(
        (dropped.0 - wrong_center.0).abs() < 6.0 && (dropped.1 - wrong_center.1).abs() < 6.0,
        "the piece stays where it was dropped: {dropped:?} vs {wrong_center:?}"
    );

    // CORRECT slot: the grab HARDENS the shadow (drag state) and lifts the
    // piece; the release pins to the slot center, swaps in the own-hue
    // GLOW, settles the scale back to 1, and the badge counts it.
    let to = center(&app, &["jw", &format!("ghost-{target0}")]);
    app.pointer_down(dropped.0, dropped.1);
    app.wait_for_timeout(Duration::from_millis(16));
    app.pointer_move(to.0 + 10.0, to.1 + 10.0);
    app.wait_for_timeout(Duration::from_millis(16));
    let (sc, sb, sd) = jigsaw_shadow(&app, 0);
    assert_eq!(
        sc,
        Some(tur_engine::core::render::brush::Color::rgba(0, 0, 0, 0xB4)),
        "dragging shadow color"
    );
    assert_eq!(sb, Some(28.0), "dragging shadow blur");
    assert_eq!(sd, Some(12.0), "dragging shadow dy");
    assert!(
        jigsaw_lift(&app, 0) > 1.001,
        "the lift eased the piece above scale 1 mid-drag"
    );
    app.pointer_move(to.0, to.1);
    app.wait_for_timeout(Duration::from_millis(16));
    app.pointer_up(to.0, to.1);
    // The 180ms settle rides the virtual clock.
    app.wait_for_timeout(Duration::from_millis(300));

    assert_eq!(
        label(&app, &["jw", "counter"]),
        "1 / 9 placed",
        "snap counts"
    );
    let (sc, sb, sd) = jigsaw_shadow(&app, 0);
    let glow = [
        0xD14747, 0xD1B347, 0x83D147, 0x47D178, 0x47BFD1, 0x4753D1, 0xA847D1, 0xD1478F, 0xD16C47,
    ][target0 as usize];
    let want = tur_engine::core::render::brush::Color::rgba(
        (glow >> 16) as u8,
        (glow >> 8) as u8,
        glow as u8,
        0x8C,
    );
    assert_eq!(sc, Some(want), "placed pieces glow their own hue");
    assert_eq!(sb, Some(18.0), "placed glow blur");
    assert_eq!(sd, Some(4.0), "placed glow dy");
    assert!(
        (jigsaw_lift(&app, 0) - 1.0).abs() < 0.001,
        "the released piece settled back to scale 1"
    );
    let snapped = center(&app, &["jw", "piece-0"]);
    assert!(
        (snapped.0 - to.0).abs() < 1.5 && (snapped.1 - to.1).abs() < 1.5,
        "the piece pinned to the slot center: {snapped:?} vs {to:?}"
    );

    // A SECOND piece snaps the same way.
    let target1 = jigsaw_target(&app, 1);
    let from1 = center(&app, &["jw", "piece-1"]);
    let to1 = center(&app, &["jw", &format!("ghost-{target1}")]);
    mouse_drag(&mut app, from1, to1, 6);
    assert_eq!(
        label(&app, &["jw", "counter"]),
        "2 / 9 placed",
        "second snap counts"
    );

    // Placed pieces refuse re-grabs.
    let pinned = center(&app, &["jw", "piece-0"]);
    let elsewhere = center(&app, &["jw", "ghost-0"]);
    mouse_drag(&mut app, pinned, elsewhere, 4);
    let after = center(&app, &["jw", "piece-0"]);
    assert!(
        (after.0 - pinned.0).abs() < 0.5 && (after.1 - pinned.1).abs() < 0.5,
        "placed pieces don't move: {after:?} vs {pinned:?}"
    );
    assert_eq!(label(&app, &["jw", "counter"]), "2 / 9 placed");

    // Shuffle re-deals: the counter resets and piece 0 returns to tray
    // slot 0 (absolute (140, 370) = play offset (10, 40) + tray slot 0).
    let (sx, sy) = center(&app, &["jw", "shuffle"]);
    app.click(sx, sy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        label(&app, &["jw", "counter"]),
        "0 / 9 placed",
        "shuffle resets"
    );
    assert!(
        app.query_element(&["jw", "snap-hl"]).is_none(),
        "no highlight after shuffle"
    );
    let home = center(&app, &["jw", "piece-0"]);
    assert!(
        (home.0 - 140.0).abs() < 1.0 && (home.1 - 370.0).abs() < 1.0,
        "piece 0 re-dealt to tray slot 0: {home:?} vs (140, 370)"
    );

    // Replayable: piece 0 snaps again after the shuffle…
    let target0b = jigsaw_target(&app, 0);
    let from0b = center(&app, &["jw", "piece-0"]);
    let to0b = center(&app, &["jw", &format!("ghost-{target0b}")]);
    mouse_drag(&mut app, from0b, to0b, 6);
    assert_eq!(
        label(&app, &["jw", "counter"]),
        "1 / 9 placed",
        "replay counts"
    );

    // …and solving the board flips the Solved! banner on at 9 / 9 (a
    // full-viewer end screen — it covers Shuffle, so the game rests there).
    assert!(
        app.query_element(&["jw", "banner"]).is_none(),
        "no banner yet"
    );
    for p in 1..9 {
        let t = jigsaw_target(&app, p);
        let from = center(&app, &["jw", &format!("piece-{p}")]);
        let to = center(&app, &["jw", &format!("ghost-{t}")]);
        mouse_drag(&mut app, from, to, 6);
    }
    assert_eq!(
        label(&app, &["jw", "counter"]),
        "9 / 9 placed",
        "solved count"
    );
    assert!(
        app.query_element(&["jw", "banner"]).is_some(),
        "the Solved! banner shows at 9 / 9"
    );

    // The end screen is FULL-VIEWER (boa parity): the win overlay — the
    // four-edge Positioned wrapping the Condition — must FILL the viewer
    // and CENTER the banner, not shrink to the banner's natural size at
    // the top-left (the round-5 audit's resolved-state MAJOR).
    let tree = app.element_tree();
    let root = tree.root_element().unwrap();
    let stack_id = tur_engine::core::element::ElementNodeId::new(root.children[0].as_u64());
    let stack = tree.get_element(stack_id).unwrap();
    assert_eq!(stack.kind().unwrap(), ElementKind::new("tur_stack"));
    let win_id =
        tur_engine::core::element::ElementNodeId::new((*stack.children.last().unwrap()).as_u64());
    let win = tree.get_element(win_id).unwrap();
    assert_eq!(
        win.kind().unwrap(),
        ElementKind::new("tur_positioned"),
        "the win overlay is the stack's last child"
    );
    let (vw, vh) = (400.0, 600.0);
    assert_eq!(
        win.computed_layout.size.width, vw,
        "the overlay fills the width"
    );
    assert_eq!(
        win.computed_layout.size.height, vh,
        "the overlay fills the height"
    );
    assert_eq!(
        win.computed_layout.offset.x, 0.0,
        "anchored at the origin x"
    );
    assert_eq!(
        win.computed_layout.offset.y, 0.0,
        "anchored at the origin y"
    );
    // The scrim (the banner card's parent Container) fills too…
    let banner_id = app.query_element(&["jw", "banner"]).unwrap();
    let banner = tree
        .get_element(tur_engine::core::element::ElementNodeId::new(
            banner_id.as_u64(),
        ))
        .unwrap();
    let scrim_id = tur_engine::core::element::ElementNodeId::new(
        banner.parent.expect("the banner's scrim parent").as_u64(),
    );
    let scrim = tree.get_element(scrim_id).unwrap();
    assert_eq!(
        scrim.computed_layout.size.width, vw,
        "the scrim fills the width"
    );
    assert_eq!(
        scrim.computed_layout.size.height, vh,
        "the scrim fills the height"
    );
    // …and the banner card sits centered in the viewer.
    let b = center(&app, &["jw", "banner"]);
    assert!(
        (b.0 - vw / 2.0).abs() < 1.0 && (b.1 - vh / 2.0).abs() < 1.0,
        "the Solved! banner is viewer-centered: {b:?} vs ({}, {})",
        vw / 2.0,
        vh / 2.0
    );
}
