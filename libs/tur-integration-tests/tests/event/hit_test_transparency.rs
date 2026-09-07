//! Flutter-parity hit-testing: only things that paint (or declare themselves
//! hit targets) absorb pointer/wheel events — invisible wrappers are
//! transparent, so hits fall through to what's behind.
//!
//! Fixtures:
//! - `hit-through-overlay` — the floating-pill pattern: a full-size invisible
//!   `SizedBox` overlay above a `ScrollView` must not steal its wheel/pointer
//!   path, while the opaque anchored pill above it still consumes clicks.
//! - `hit-test-opaqueness` — the absorption matrix: a colored `Container`
//!   absorbs; a decoration-less `Container` passes through; a `PointerInteract`
//!   (opaque default) catches invisible hits; a translucent `MouseRegion`
//!   joins the path without blocking.

use tur_engine::builtin_plugins::scroll::ScrollViewElement;
use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

fn setup(name: &str) -> TurTestApp {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle(name).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    app
}

fn scroll_offset(app: &TurTestApp, key: &[&str]) -> f64 {
    let id = app.query_element(key).unwrap();
    app.with_element(ElementNodeId::new(id.as_u64()), |e| {
        e.cast::<ScrollViewElement>().unwrap().scroll_offset()
    })
    .unwrap()
}

// ── hit-through-overlay ─────────────────────────────────────────────────

/// THE reported bug: wheel over the invisible full-size overlay must reach
/// the ScrollView beneath it (Flutter: an unpainted wrapper is transparent
/// to hit-testing).
#[test]
fn wheel_passes_through_invisible_overlay_to_scrollview() {
    let mut app = setup("hit-through-overlay");

    app.wheel(0.0, 100.0, 200.0, 150.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        scroll_offset(&app, &["sv"]),
        100.0,
        "wheel over the invisible overlay must scroll the ScrollView beneath"
    );
}

/// Pointer-down over the invisible overlay must still fire on the
/// PointerInteract beneath (it stays in the hit path as an ancestor of the
/// absorbing scroll view).
#[test]
fn pointer_down_beneath_invisible_overlay_fires() {
    let mut app = setup("hit-through-overlay");

    app.pointer_down(200.0, 150.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    let downs: u32 = app
        .eval_js("globalThis.__getBeneathDowns()")
        .parse()
        .unwrap();
    assert_eq!(downs, 1, "pointer down must pass through the overlay");
}

/// The floating pill is anchored bottom-right via `Positioned` edges
/// (400x600 stack, 64x28 pill, right/bottom 8 → origin (328, 564)).
#[test]
fn pill_is_anchored_bottom_right() {
    let app = setup("hit-through-overlay");

    let pill = app.query_element(&["pill"]).unwrap();
    let bounds = app
        .get_element_absolute_bounds(ElementNodeId::new(pill.as_u64()))
        .unwrap();

    assert_eq!(bounds.left, 400.0 - 8.0 - 64.0);
    assert_eq!(bounds.top, 600.0 - 8.0 - 28.0);
    assert_eq!(bounds.right - bounds.left, 64.0);
    assert_eq!(bounds.bottom - bounds.top, 28.0);
}

/// The pill itself (opaque gesture target + painted surface) still consumes
/// clicks — the passthrough fix must not make everything transparent.
#[test]
fn pill_clicks_over_overlay_still_fire() {
    let mut app = setup("hit-through-overlay");

    let pill = app.query_element(&["pill"]).unwrap();
    let (cx, cy) = app
        .get_element_absolute_bounds(ElementNodeId::new(pill.as_u64()))
        .unwrap()
        .center();
    app.click(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);

    let clicks: u32 = app.eval_js("globalThis.__getPillClicks()").parse().unwrap();
    assert_eq!(clicks, 1, "the opaque pill must still receive clicks");
}

// ── hit-test-opaqueness ─────────────────────────────────────────────────

fn beneath_downs(app: &TurTestApp) -> u32 {
    app.eval_js("globalThis.__getBeneathDowns()")
        .parse()
        .unwrap()
}

/// A painted surface (Container with color) absorbs hits — DecoratedBox /
/// ColoredBox parity. The invisible gesture target behind it must NOT fire.
#[test]
fn colored_container_absorbs_hits() {
    let mut app = setup("hit-test-opaqueness");

    app.pointer_down(100.0, 100.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        beneath_downs(&app),
        0,
        "a colored Container absorbs the hit — nothing behind it fires"
    );
}

/// A decoration-less Container is transparent, and a translucent
/// MouseRegion joins the path without absorbing: a pointer-down over the
/// right half (plain container + translucent region, no painted surface)
/// falls through to the opaque PointerInteract beneath.
#[test]
fn plain_container_and_translucent_region_pass_through() {
    let mut app = setup("hit-test-opaqueness");

    app.pointer_down(300.0, 300.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        beneath_downs(&app),
        1,
        "plain Container + translucent MouseRegion must pass the hit through"
    );
}

/// Same passthrough on the left half (plain container only — no region),
/// isolating the decoration-less-Container transparency from the region's
/// translucency.
#[test]
fn plain_container_passes_through() {
    let mut app = setup("hit-test-opaqueness");

    app.pointer_down(100.0, 300.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(beneath_downs(&app), 1);
}
