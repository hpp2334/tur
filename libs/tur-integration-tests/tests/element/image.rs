use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

#[test]
fn image_with_explicit_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("image-basic").unwrap();

    let image_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let container = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(
            container.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );
        assert_eq!(container.children.len(), 1);

        let image = tree.get_element(ElementNodeId::new(container.children[0].as_u64())).unwrap();
        assert_eq!(
            image.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_image")
        );
        assert_eq!(image.children.len(), 0);
        image.id
    };

    app.render();
    let rt = app.element_tree();
    let image_node = rt.get_element(image_id).unwrap();
    assert_eq!(image_node.computed_layout.size.width, 200.0);
    assert_eq!(image_node.computed_layout.size.height, 100.0);
}
