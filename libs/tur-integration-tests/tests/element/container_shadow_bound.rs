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
use tur::{ ctx_bridge, mount };
use tur_kit::{ Container, MutationCtx, Readable, Source, source };

entry fn start() -> u64 {
    // The channels mint in order: color, blur, dy (the probe entries
    // address them by the start answer + offset).
    let color: Readable<u64> = source<u64>(0);
    let blur: Readable<f64> = source<f64>(10.0);
    let dy: Readable<f64> = source<f64>(4.0);
    // The boot glow (the ctx face at start).
    MutationCtx.over(ctx_bridge()).set<u64>(color, 0xFF0000FFu64);
    let card = Container()
        .width_height(60.0, 40.0)
        .color(0x222222FFu64)
        .shadow_color_bound(color)
        .shadow_blur_bound(blur)
        .shadow_dy_bound(dy)
        .query_key("card")
        .build();
    mount(card);
    return color.atom_id();
}

// Re-tint the glow (the placed-piece hue swap) — a brush write only; the
// element must repaint without remounting.
entry fn retint(atom: u64, _b: f64) {
    let color = Source<u64>.of(ctx_bridge(), atom, false, 1);
    MutationCtx.over(ctx_bridge()).set<u64>(color, 0x00FF00FFu64);
}

entry fn resteepen(atom: u64, _b: f64) {
    let dy = Source<f64>.of(ctx_bridge(), atom, false, 1);
    MutationCtx.over(ctx_bridge()).set<f64>(dy, 12.0);
}
"#;

const STATIC_SHADOW_RUT: &str = r#"
use tur::mount;
use tur_kit::{ Container, Mutation, MutationCtx, Readable, Source, source };

entry fn start() -> u64 {
    let dy: Readable<f64> = source<f64>(12.0);
    let card = Container()
        .width_height(60.0, 40.0)
        .color(0x222222FFu64)
        .shadow(0xABCDEFFFu64, 24.0, 0.0, 4.0)
        .shadow_dy_bound(dy)
        .query_key("card")
        .build();
    mount(card);
    return dy.atom_id();
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
        assert!(
            e.cast::<ContainerElement>().is_some(),
            "card is a container"
        );
    })
    .unwrap();
    (app, card)
}

#[test]
fn bound_shadow_channels_paint_their_atoms() {
    let (mut app, card) = setup(SHADOW_BOUND_RUT);

    app.with_element(card, |e| {
        let c = e.cast::<ContainerElement>().unwrap();
        assert_eq!(
            c.painted_shadow_color(),
            Some(tur_engine::core::render::brush::Color::rgba(
                0xFF, 0x00, 0x00, 0xFF
            ))
        );
        assert_eq!(c.painted_shadow_blur(), Some(10.0));
        assert_eq!(c.painted_shadow_dy(), Some(4.0));
    })
    .unwrap();
}

#[test]
fn shadow_atom_writes_repaint_without_remount() {
    let (mut app, card) = setup(SHADOW_BOUND_RUT);

    let color_atom = app.rut_start_answer();
    app.call_rut_entry("retint", color_atom, 0.0).unwrap();
    app.call_rut_entry("resteepen", color_atom + 2, 0.0)
        .unwrap(); // the dy channel mints third
    app.wait_for_timeout(std::time::Duration::ZERO);

    // The SAME element id — a rebuild would have minted a new one.
    app.with_element(card, |e| {
        let c = e.cast::<ContainerElement>().unwrap();
        assert_eq!(
            c.painted_shadow_color(),
            Some(tur_engine::core::render::brush::Color::rgba(
                0x00, 0xFF, 0x00, 0xFF
            ))
        );
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
        assert_eq!(
            c.painted_shadow_color(),
            Some(tur_engine::core::render::brush::Color::rgba(
                0xAB, 0xCD, 0xEF, 0xFF
            ))
        );
        assert_eq!(c.painted_shadow_blur(), Some(24.0));
        assert_eq!(c.painted_shadow_dy(), Some(12.0));
    })
    .unwrap();
}
