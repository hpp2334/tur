use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

#[test]
fn sized_box_fixed_dimensions() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("sized-box-basic").unwrap();

    let sb_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let sb = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(
            sb.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );
        assert_eq!(sb.children.len(), 1);

        let text = tree.get_element(ElementNodeId::new(sb.children[0].as_u64())).unwrap();
        assert_eq!(
            text.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_paragraph")
        );

        sb.id
    };

    app.render();
    let rt = app.element_tree();
    let sb_node = rt.get_element(ElementNodeId::new(sb_id.as_u64())).unwrap();
    assert_eq!(sb_node.computed_layout.size.width, 100.0);
    assert_eq!(sb_node.computed_layout.size.height, 50.0);
}
