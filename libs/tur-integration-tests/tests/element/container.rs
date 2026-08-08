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

    app.render();
    let rt = app.element_tree();
    let container_node = rt.get_element(container_id).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 132.0);
    assert_eq!(container_node.computed_layout.size.height, 132.0);
}

#[test]
fn container_update_clears_removed_props() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("container-update-clear-prop").unwrap();
    app.render();

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
    app.render();

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

    app.render();
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

    app.render();
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

    app.render();
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

    app.render();
    let rt = app.element_tree();
    let container_node = rt.get_element(container_id).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 200.0);
    assert_eq!(container_node.computed_layout.size.height, 200.0);
}
