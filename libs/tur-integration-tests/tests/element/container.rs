use tur_engine::core::element::ElementKind;
use tur_engine::elements::ContainerElement;
use tur_shared::BorderPosition;
use tur_integration_tests::TurTestApp;

#[test]
fn container_with_padding() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("container-basic").unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let container = tree.get(root.children[0]).unwrap();
        assert_eq!(
            container.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );
        assert_eq!(container.children.len(), 1);

        let sb = tree.get(container.children[0]).unwrap();
        assert_eq!(
            sb.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        container.id
    };

    app.render();
    let rt = app.element_tree();
    let container_node = rt.get(container_id).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 132.0);
    assert_eq!(container_node.computed_layout.size.height, 132.0);
}

#[test]
fn container_with_border() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("container-border").unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let container = tree.get(root.children[0]).unwrap();
        assert_eq!(
            container.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );
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
    let container_node = rt.get(container_id).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 200.0);
    assert_eq!(container_node.computed_layout.size.height, 200.0);
}

#[test]
fn container_with_shadow() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("container-shadow").unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let container = tree.get(root.children[0]).unwrap();
        assert_eq!(
            container.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );
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
    let container_node = rt.get(container_id).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 200.0);
    assert_eq!(container_node.computed_layout.size.height, 200.0);
}
