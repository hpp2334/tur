use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn image_with_explicit_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("image-basic").unwrap();

    let image_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let container = tree.get(root.children[0]).unwrap();
        assert_eq!(
            container.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );
        assert_eq!(container.children.len(), 1);

        let image = tree.get(container.children[0]).unwrap();
        assert_eq!(
            image.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_image")
        );
        assert_eq!(image.children.len(), 0);
        image.id
    };

    app.render();
    let rt = app.element_tree();
    let image_node = rt.get(image_id).unwrap();
    assert_eq!(image_node.computed_layout.size.width, 200.0);
    assert_eq!(image_node.computed_layout.size.height, 100.0);
}
