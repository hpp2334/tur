//! Bound shadow channels on the box family — `shadow_color_bound` /
//! `shadow_blur_bound` / `shadow_dy_bound`.
//!
//! The jigsaw resolved-state port needs per-state piece glows (loose black
//! soft shadow → dragging hard shadow → placed own-hue glow) and the
//! implicit-animations port needs a shadow blur that eases 16↔32 — both
//! ride reactive atoms, not rebuilds. The `ContainerView` fields were
//! already `Val`-backed (`shadow_color` / `shadow_blur`); these rows are
//! the reactive twins of `box_radius_bound`. The offset dy channel is new
//! (`shadow_offset` was factory-static): the bound dy wins over the static
//! tuple's y while drag states animate `[0,4] → [0,12]`.

use tur_engine::builtin_plugins::layout::ContainerElement;
use tur_integration_tests::TurTestApp;

const SHADOW_BOUND_RUT: &str = r#"
use tur::{ mount, rs_set_brush, rs_set_f64, rs_source_f64, stf_put, stf_take };
use tur_kit::{ Column, Container };

let K_COLOR: u64 = 1;
let K_BLUR: u64 = 2;
let K_DY: u64 = 3;

entry fn start() -> u64 {
    let color = rs_source_f64();
    let blur = rs_source_f64();
    let dy = rs_source_f64();
    rs_set_brush(color, 0xFF0000FFu64);
    rs_set_f64(blur, 10.0);
    rs_set_f64(dy, 4.0);
    stf_put(K_COLOR, color as f64);
    stf_put(K_BLUR, blur as f64);
    stf_put(K_DY, dy as f64);
    let card = Container()
        .width_height(60.0, 40.0)
        .color(0x222222FFu64)
        .shadow_color_bound(color)
        .shadow_blur_bound(blur)
        .shadow_dy_bound(dy)
        .query_key("card")
        .build();
    mount(card);
    return 0;
}

// Re-tint the glow (the placed-piece hue swap) — an atom write only; the
// element must repaint without remounting.
entry fn retint(_a: u64, _b: f64) {
    let color = stf_take(K_COLOR) as u64;
    stf_put(K_COLOR, color as f64);
    rs_set_brush(color, 0x00FF00FFu64);
}

entry fn resteepen(_a: u64, _b: f64) {
    let dy = stf_take(K_DY) as u64;
    stf_put(K_DY, dy as f64);
    rs_set_f64(dy, 12.0);
}
"#;

const STATIC_SHADOW_RUT: &str = r#"
use tur::{ mount, rs_set_f64, rs_source_f64, stf_put };
use tur_kit::{ Container };

let K_DY: u64 = 3;

entry fn start() -> u64 {
    let dy = rs_source_f64();
    rs_set_f64(dy, 12.0);
    stf_put(K_DY, dy as f64);
    let card = Container()
        .width_height(60.0, 40.0)
        .color(0x222222FFu64)
        .shadow(0xABCDEFFFu64, 24.0, 0.0, 4.0)
        .shadow_dy_bound(dy)
        .query_key("card")
        .build();
    mount(card);
    return 0;
}
"#;

fn setup(source: &str) -> (TurTestApp, tur_engine::core::element::ElementNodeId) {
    let mut app = TurTestApp::new(300.0, 200.0).unwrap();
    app.load_rut_module(source).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let tree = app.element_tree();
    let root_snap = tree.root_element().unwrap();
    let card = tur_engine::core::element::ElementNodeId::new(root_snap.children[0].as_u64());
    app.with_element(card, |e| {
        assert!(e.cast::<ContainerElement>().is_some(), "card is a container");
    })
    .unwrap();
    (app, card)
}

#[test]
fn bound_shadow_channels_paint_their_atoms() {
    let (mut app, card) = setup(SHADOW_BOUND_RUT);

    app.with_element(card, |e| {
        let c = e.cast::<ContainerElement>().unwrap();
        assert_eq!(c.painted_shadow_color(), Some(tur_engine::core::render::brush::Color::rgba(0xFF, 0x00, 0x00, 0xFF)));
        assert_eq!(c.painted_shadow_blur(), Some(10.0));
        assert_eq!(c.painted_shadow_dy(), Some(4.0));
    })
    .unwrap();
}

#[test]
fn shadow_atom_writes_repaint_without_remount() {
    let (mut app, card) = setup(SHADOW_BOUND_RUT);

    app.call_rut_entry("retint", 0, 0.0).unwrap();
    app.call_rut_entry("resteepen", 0, 0.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // The SAME element id — a rebuild would have minted a new one.
    app.with_element(card, |e| {
        let c = e.cast::<ContainerElement>().unwrap();
        assert_eq!(c.painted_shadow_color(), Some(tur_engine::core::render::brush::Color::rgba(0x00, 0xFF, 0x00, 0xFF)));
        assert_eq!(c.painted_shadow_dy(), Some(12.0));
        assert_eq!(c.painted_shadow_blur(), Some(10.0), "blur atom untouched");
    })
    .unwrap();
}

#[test]
fn bound_dy_overrides_the_static_offset_y() {
    let (mut app, card) = setup(STATIC_SHADOW_RUT);

    app.with_element(card, |e| {
        let c = e.cast::<ContainerElement>().unwrap();
        // Static .shadow() supplies color/blur/dx; the bound dy wins on y.
        assert_eq!(c.painted_shadow_color(), Some(tur_engine::core::render::brush::Color::rgba(0xAB, 0xCD, 0xEF, 0xFF)));
        assert_eq!(c.painted_shadow_blur(), Some(24.0));
        assert_eq!(c.painted_shadow_dy(), Some(12.0));
    })
    .unwrap();
}
