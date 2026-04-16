use tur_integration_tests::TurTestApp;
use tur_render_tree::RenderNodeId;
use tur_widget::WidgetKind;

#[test]
fn positioned_with_left_top() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("positioned-basic").unwrap();

    let pos_id = {
        let tree = app.widget_tree();
        let root = tree.root().unwrap();
        let stack = tree.get(root.children[0]).unwrap();
        assert_eq!(stack.kind, WidgetKind::Stack);
        assert_eq!(stack.children.len(), 1);

        let positioned = tree.get(stack.children[0]).unwrap();
        assert_eq!(positioned.kind, WidgetKind::Positioned);
        assert_eq!(positioned.children.len(), 1);

        let sb = tree.get(positioned.children[0]).unwrap();
        assert_eq!(sb.kind, WidgetKind::SizedBox);

        positioned.id.as_u64()
    };

    let rt = app.render_tree();
    let pos_node = rt.get(RenderNodeId::new(pos_id)).unwrap();
    assert_eq!(pos_node.computed_layout.offset.x, 10.0);
    assert_eq!(pos_node.computed_layout.offset.y, 20.0);
}
