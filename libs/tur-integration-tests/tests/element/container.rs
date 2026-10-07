use tur_engine::builtin_plugins::layout::ContainerElement;
use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::core::layout::BorderPosition;
use tur_integration_tests::TurTestApp;

#[test]
fn container_with_padding() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("container-basic").unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let container = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        assert_eq!(container.kind().unwrap(), ElementKind::new("tur_container"));
        assert_eq!(container.children.len(), 1);

        let sb = tree
            .get_element(ElementNodeId::new(container.children[0].as_u64()))
            .unwrap();
        assert_eq!(sb.kind().unwrap(), ElementKind::new("tur_container"));

        container.id
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();
    let container_node = rt.get_element(container_id).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 132.0);
    assert_eq!(container_node.computed_layout.size.height, 132.0);
}

#[test]
fn container_update_clears_removed_props() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("container-update-clear-prop").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let checkbox_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let container = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        let pointer = tree
            .get_element(ElementNodeId::new(container.children[0].as_u64()))
            .unwrap();
        let checkbox = tree
            .get_element(ElementNodeId::new(pointer.children[0].as_u64()))
            .unwrap();
        assert_eq!(checkbox.kind().unwrap(), ElementKind::new("tur_container"),);
        checkbox.id
    };

    app.with_element(checkbox_id, |el| {
        let c = el.cast::<ContainerElement>().unwrap();
        eprintln!("[test] before toggle: color={:?}", c.color());
        assert!(c.color().is_some(), "checked state should have color");
    });

    let (cx, cy) = app
        .get_element_absolute_bounds(checkbox_id)
        .unwrap()
        .center();
    eprintln!("[test] clicking at ({}, {})", cx, cy);
    app.click(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(checkbox_id, |el| {
        let c = el.cast::<ContainerElement>().unwrap();
        eprintln!(
            "[test] after toggle: color={:?}, border_color={:?}",
            c.color(),
            c.border_color()
        );
        assert!(
            c.color().is_none(),
            "unchecked state should NOT have color, got {:?}",
            c.color()
        );
    });
}

#[test]
fn container_with_border() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("container-border").unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let container = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        assert_eq!(container.kind().unwrap(), ElementKind::new("tur_container"));
        assert_eq!(container.children.len(), 1);
        container.id
    };

    app.with_element(container_id, |el| {
        let c = el.cast::<ContainerElement>().unwrap();
        assert_eq!(c.width(), Some(200.0));
        assert_eq!(c.height(), Some(200.0));
        assert_eq!(c.padding(), Some(16.0));
        assert!(c.border_color().is_some());
        assert_eq!(c.border_width(), Some(2.0));
        assert_eq!(c.border_radius(), Some(8.0));
        assert_eq!(c.border_position(), BorderPosition::Inside);
    });

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();
    let container_node = rt.get_element(container_id).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 200.0);
    assert_eq!(container_node.computed_layout.size.height, 200.0);
}

#[test]
fn container_padding_offsets_child() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("container-padding-offset").unwrap();

    let (container_id, row_id, sb_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let container = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        let row = tree
            .get_element(ElementNodeId::new(container.children[0].as_u64()))
            .unwrap();
        let sb = tree
            .get_element(ElementNodeId::new(row.children[0].as_u64()))
            .unwrap();
        (container.id, row.id, sb.id)
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let container = rt.get_element(container_id).unwrap();
    assert_eq!(container.computed_layout.size.width, 200.0);
    assert_eq!(container.computed_layout.size.height, 100.0);

    let row = rt.get_element(row_id).unwrap();
    assert_eq!(
        row.computed_layout.offset.x, 20.0,
        "Row should be offset by padding=20"
    );
    assert_eq!(
        row.computed_layout.offset.y, 20.0,
        "Row should be offset by padding=20"
    );

    let sb = rt.get_element(sb_id).unwrap();
    assert_eq!(sb.computed_layout.offset.x, 0.0);
    assert_eq!(sb.computed_layout.offset.y, 0.0);
}

#[test]
fn container_with_explicit_size_in_flex() {
    let mut app = TurTestApp::new(828.0, 864.0).unwrap();
    app.load_bundle("container-flex-sized").unwrap();

    let btn_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let col = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        let row = tree
            .get_element(ElementNodeId::new(col.children[0].as_u64()))
            .unwrap();
        let container = tree
            .get_element(ElementNodeId::new(row.children[0].as_u64()))
            .unwrap();
        container.id
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let btn = rt.get_element(btn_id).unwrap();
    assert_eq!(
        btn.computed_layout.size.width, 100.0,
        "container width should be 100, got {}",
        btn.computed_layout.size.width,
    );
    assert_eq!(
        btn.computed_layout.size.height, 44.0,
        "container height should be 44, got {}",
        btn.computed_layout.size.height,
    );
}
#[test]
fn container_with_shadow() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("container-shadow").unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let container = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        assert_eq!(container.kind().unwrap(), ElementKind::new("tur_container"));
        assert_eq!(container.children.len(), 1);
        container.id
    };

    app.with_element(container_id, |el| {
        let c = el.cast::<ContainerElement>().unwrap();
        assert_eq!(c.width(), Some(200.0));
        assert_eq!(c.height(), Some(200.0));
        assert_eq!(c.border_radius(), Some(8.0));
        assert!(c.shadow_color().is_some());
        assert_eq!(c.shadow_offset(), Some((4.0, 4.0)));
        assert_eq!(c.shadow_blur(), Some(12.0));
    });

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();
    let container_node = rt.get_element(container_id).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 200.0);
    assert_eq!(container_node.computed_layout.size.height, 200.0);
}

// ---------------------------------------------------------------------------
// box_radius_bound — the reactive corner radius (`Container().radius_bound`
// mirrors `width_bound`/`height_bound`/`color_bound`; animated corner
// radius rides a derive of the progress atom).
// ---------------------------------------------------------------------------

const RADIUS_BOUND_RUT: &str = r#"
use tur::{ ctx_bridge, mount };
use tur_kit::{ Container, Mutation, MutationCtx, Readable, Source, source };

entry fn start() -> u64 {
    let r: Readable<f64> = source<f64>(8.0);
    let card = Container().width_height(100.0, 100.0).radius_bound(r).query_key("rb/box").build();
    mount(card);
    return r.atom_id();
}

entry fn probe_r(atom: u64, b: f64) {
    let r = Source<f64>.of(ctx_bridge(), atom, false, 1);
    MutationCtx.over(ctx_bridge()).set<f64>(r, b);
}
"#;

#[test]
fn radius_bound_resolves_through_the_live_atom() {
    let mut app = TurTestApp::new(200.0, 200.0).unwrap();
    app.load_rut_module(RADIUS_BOUND_RUT).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let box_id = ElementNodeId::new(app.query_element(&["rb", "box"]).unwrap().as_u64());
    app.with_element(box_id, |el| {
        let c = el.cast::<ContainerElement>().unwrap();
        assert_eq!(c.border_radius(), Some(8.0), "the atom's initial value");
    });

    // The atom swap re-resolves the radius through layout (the subscribe
    // → relayout rail; painting carries the reactive value).
    app.call_rut_entry("probe_r", app.rut_start_answer(), 20.0)
        .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.with_element(box_id, |el| {
        let c = el.cast::<ContainerElement>().unwrap();
        assert_eq!(c.border_radius(), Some(20.0), "radius follows the atom");
    });
}
