use tur_engine::core::element::{ElementKind, ElementNodeId, FragmentNodeId};
use tur_integration_tests::TurTestApp;

// Regression: a content-sized Row containing an `Each` (a vertical,
// MainAxisSize::Max flex) must not inflate to the parent Column's full height.
// Before the fix, the `Each` greedily consumed the Row's incoming maxHeight
// (the full Column height) and reported it back as its own height, so the Row's
// cross-axis (height) became the full Column height — starving the SizedBox
// marker and the Expanded below it to zero.
#[test]
fn row_with_each_does_not_inflate() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("row-with-each").unwrap();

    let (col_id, row_id, each_id, marker_id, expanded_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let col = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(
            col.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        let row = tree.get_element(ElementNodeId::new(col.children[0].as_u64())).unwrap();
        let marker = tree.get_element(ElementNodeId::new(col.children[1].as_u64())).unwrap();
        let expanded = tree.get_element(ElementNodeId::new(col.children[2].as_u64())).unwrap();
        // The Each sits after the sized box inside the Row. It is now a
        // Fragment (no layout box of its own), so take its id directly
        // instead of looking it up as an element.
        let each_id = row.children[1];
        (col.id, row.id, each_id, marker.id, expanded.id)
    };

    app.render();
    let rt = app.element_tree();

    let row = rt.get_element(row_id).unwrap();
    let row_h = row.computed_layout.size.height;
    // The Each is a Fragment (no layout box); its Text children are spliced
    // into the Row. Read the first such child's height as a proxy for the
    // debug log.
    let each_h = rt
        .get_fragment(FragmentNodeId::new(each_id.as_u64()))
        .and_then(|f| f.children.first().copied())
        .and_then(|cid| rt.get_element(ElementNodeId::new(cid.as_u64())))
        .map(|n| n.computed_layout.size.height)
        .unwrap_or(0.0);
    eprintln!("row height={row_h}, each height={each_h}");
    // The Row's tallest child is the 16px sized box (Text renders ~16-18px
    // tall too). Either way the Row must be a small content height — NOT the
    // full 600px Column height.
    assert!(
        row_h < 60.0,
        "Row containing an Each should size to content (<60px), got {row_h}px (full height = inflation bug)"
    );

    let marker = rt.get_element(marker_id).unwrap();
    // Marker must sit just below the Row, not be starved to zero or pushed off.
    assert_eq!(
        marker.computed_layout.size.height, 10.0,
        "marker SizedBox must keep its 10px height"
    );
    assert!(
        marker.computed_layout.offset.y < 60.0,
        "marker must be near the top (just under the Row), got y={} — siblings starved",
        marker.computed_layout.offset.y
    );

    let expanded = rt.get_element(expanded_id).unwrap();
    assert!(
        expanded.computed_layout.size.height > 400.0,
        "Expanded must fill the remaining Column height, got {} — starved by inflated Row",
        expanded.computed_layout.size.height
    );

    let _ = (col_id, each_id);
}
