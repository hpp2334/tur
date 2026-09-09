use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

// Flutter reports the degenerate cases below as layout errors ("RenderFlex
// children have non-zero flex but incoming height constraints are unbounded";
// stretch under unbounded cross). tur degrades gracefully instead: Stretch
// falls back to loose cross constraints, and flex children under an
// unbounded main axis lay out as INFLEXIBLE (Flutter's `canFlex == false`
// mechanics — unbounded main passed through), never zero-size slots — and
// no infinite size may ever leak upward.
#[test]
fn flex_degenerate_unbounded_cases_degrade_finitely() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import {
            mount,
            Column,
            CrossAxisAlignment,
            Expanded,
            Row,
            SizedBox,
        } from "tur:std";

        mount(Column()
    .children([
                // Stretch Row under an unbounded cross axis (non-flex child
                // of a Column): Stretch degrades to loose cross.
                Row()
                    .crossAlignment(CrossAxisAlignment.Stretch)
                    .queryKey(["stretch-row"])
                    .children([SizedBox()
    .width(50)
    .build()])
                    .build(),
                // Expanded inside a Column with unbounded height: the flex
                // child lays out as inflexible (natural size), never a
                // zero slot and never infinity.
                Column()
                    .queryKey(["flex-col"])
                    .children([Expanded()
    .child(SizedBox()
     .height(50)
     .build())
    .build()])
                    .build(),
            ])
    .build());
    "#,
    )
    .unwrap();

    let (stretch_row_id, flex_col_id, expanded_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let outer = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        let stretch_row = tree
            .get_element(ElementNodeId::new(outer.children[0].as_u64()))
            .unwrap();
        let flex_col = tree
            .get_element(ElementNodeId::new(outer.children[1].as_u64()))
            .unwrap();
        assert_eq!(stretch_row.children.len(), 1);
        (stretch_row.id, flex_col.id, flex_col.children[0])
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let stretch_row = rt.get_element(stretch_row_id).unwrap();
    assert!(
        stretch_row.computed_layout.size.height.is_finite(),
        "Stretch under an unbounded cross axis must not leak infinite sizes, got {:?}",
        stretch_row.computed_layout.size
    );
    assert_eq!(stretch_row.computed_layout.size.height, 0.0);

    let flex_col = rt.get_element(flex_col_id).unwrap();
    assert!(
        flex_col.computed_layout.size.height.is_finite(),
        "flex children under unbounded main must not leak infinite sizes, got {:?}",
        flex_col.computed_layout.size
    );
    assert_eq!(
        flex_col.computed_layout.size.height, 50.0,
        "under an unbounded main axis the flex child lays out as inflexible \
         (Flutter `canFlex == false`): the Column degenerates to its content"
    );

    let expanded_inner_id = {
        let expanded = rt
            .get_element(ElementNodeId::new(expanded_id.as_u64()))
            .unwrap();
        ElementNodeId::new(expanded.children[0].as_u64())
    };
    let expanded_child = rt.get_element(expanded_inner_id).unwrap();
    assert_eq!(
        expanded_child.computed_layout.size.height, 50.0,
        "the flex-item child itself lays out with unbounded main (natural \
         size), not a collapsed zero slot"
    );
}

#[test]
fn row_main_alignment_start() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("row-basic").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        (row.children[0], row.children[1])
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.offset.x, 0.0);
    assert_eq!(sb1.computed_layout.size.width, 50.0);

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.offset.x, 50.0);
    assert_eq!(sb2.computed_layout.size.width, 30.0);
}

#[test]
fn row_main_alignment_center() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("flex-row-main-center").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        (row.children[0], row.children[1])
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.offset.x, 160.0);

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.offset.x, 210.0);
}

#[test]
fn row_main_alignment_end() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("flex-row-main-end").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        (row.children[0], row.children[1])
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.offset.x, 320.0);

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.offset.x, 370.0);
}

#[test]
fn row_cross_alignment_stretch() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("flex-row-cross-stretch").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        (row.children[0], row.children[1])
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.size.height, 600.0);
    assert_eq!(sb1.computed_layout.offset.y, 0.0);

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.size.height, 600.0);
    assert_eq!(sb2.computed_layout.offset.y, 0.0);
}

#[test]
fn row_with_expanded() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("flex-row-expanded").unwrap();

    let (sb1_id, expanded_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        (row.children[0], row.children[1])
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.size.width, 50.0);
    assert_eq!(sb1.computed_layout.offset.x, 0.0);

    let expanded = rt
        .get_element(ElementNodeId::new(expanded_id.as_u64()))
        .unwrap();
    assert_eq!(expanded.computed_layout.size.width, 350.0);
    assert_eq!(expanded.computed_layout.offset.x, 50.0);
}

#[test]
fn nested_sidebar_layout() {
    let mut app = TurTestApp::new(800.0, 600.0).unwrap();
    app.load_bundle("flex-nested-sidebar").unwrap();

    let (sidebar_id, content_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        (row.children[0], row.children[1])
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let sidebar = rt
        .get_element(ElementNodeId::new(sidebar_id.as_u64()))
        .unwrap();
    assert_eq!(sidebar.computed_layout.size.width, 200.0);
    assert_eq!(sidebar.computed_layout.offset.x, 0.0);
    assert_eq!(sidebar.computed_layout.size.height, 600.0);

    let content = rt
        .get_element(ElementNodeId::new(content_id.as_u64()))
        .unwrap();
    assert_eq!(content.computed_layout.offset.x, 200.0);
    assert_eq!(content.computed_layout.size.height, 600.0);
}

#[test]
fn todolist_sidebar_at_left() {
    let mut app = TurTestApp::new(800.0, 600.0).unwrap();
    app.load_bundle("flex-todolist-sidebar").unwrap();

    let (sidebar_id, content_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        (row.children[0], row.children[1])
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let sidebar = rt
        .get_element(ElementNodeId::new(sidebar_id.as_u64()))
        .unwrap();
    assert_eq!(
        sidebar.computed_layout.offset.x, 0.0,
        "sidebar should be at left (x=0)"
    );
    assert_eq!(sidebar.computed_layout.size.width, 200.0);

    let content = rt
        .get_element(ElementNodeId::new(content_id.as_u64()))
        .unwrap();
    assert_eq!(
        content.computed_layout.offset.x, 200.0,
        "content should be at x=200"
    );
}
