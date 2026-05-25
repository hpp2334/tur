use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn svg_with_explicit_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("svg-basic").unwrap();

    let svg_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let container = tree.get(root.children[0]).unwrap();
        assert_eq!(
            container.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );
        assert_eq!(container.children.len(), 1);

        let svg = tree.get(container.children[0]).unwrap();
        assert_eq!(
            svg.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_svg")
        );
        assert_eq!(svg.children.len(), 0);
        svg.id
    };

    app.render();
    let rt = app.element_tree();
    let svg_node = rt.get(svg_id).unwrap();
    assert_eq!(svg_node.computed_layout.size.width, 200.0);
    assert_eq!(svg_node.computed_layout.size.height, 200.0);
}
