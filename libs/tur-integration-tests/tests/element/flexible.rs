use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

/// Shared fixture: a 400-wide Row holding a fixed 100-wide Container and a
/// flex item wrapping a Text — the flex slot is therefore exactly 300.
/// Returns the Text node id (root → row → flex item → text).
fn setup_row_flex_item(flex_item_js: &str) -> (TurTestApp, ElementNodeId, ElementNodeId) {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    let source = format!(
        r#"
        import {{ mount, Row, Container, Text, FlexFit, Expanded, Flexible }} from "tur:std";
        mount(Row()
            .queryKey(["row"])
            .children([
                Container().width(100).height(40).build(),
                {flex_item_js},
            ])
            .build());
        "#,
    );
    app.eval_module_source(&source).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let (row_id, text_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        assert_eq!(row.kind().unwrap(), ElementKind::new("tur_flex"));
        let flex_item = tree
            .get_element(ElementNodeId::new(row.children[1].as_u64()))
            .unwrap();
        assert_eq!(
            flex_item.kind().unwrap(),
            ElementKind::new("tur_flex_item"),
            "the second row child must be the flex item"
        );
        let text = tree
            .get_element(ElementNodeId::new(flex_item.children[0].as_u64()))
            .unwrap();
        assert_eq!(text.kind().unwrap(), ElementKind::new("tur_paragraph"));
        (row.id, text.id)
    };
    (app, row_id, text_id)
}

/// `Flexible` is `FlexFit.loose` (Flutter parity): the child may be at most
/// its slot (300) — so a longer Text ellipsizes AT the slot, the true
/// available width. This is the Flutter-idiomatic shape for ellipsizing
/// labels in a Row (a bare non-flex child gets an unbounded main axis, in
/// both Flutter and tur, and cannot ellipsize at all).
#[test]
fn flexible_loose_fit_ellipsizes_text_at_slot() {
    let (app, row_id, text_id) = setup_row_flex_item(
        r#"Flexible({ flex: 1 })
                .child(Text({
                    text: "A very long label that must ellipsize inside its slot",
                })
                    .fontSize(14)
                    .maxLines(1)
                    .overflow("ellipsis")
                    .build())
                .build()"#,
    );

    let rt = app.element_tree();
    let row = rt.get_element(row_id).unwrap();
    let text = rt.get_element(text_id).unwrap();
    assert_eq!(
        row.computed_layout.size.width, 400.0,
        "Row with MainAxisSize.max fills the bounded incoming width"
    );
    let w = text.computed_layout.size.width;
    assert!(
        w <= 300.0,
        "loose fit caps the child at its slot (300), got {w}"
    );
    assert!(
        w >= 250.0,
        "the text must actually USE the slot budget (ellipsize near 300), got {w} \
         — a premature cut (e.g. at slot/dpr) is the reported bug shape"
    );
    // Single line.
    assert!(text.computed_layout.size.height < 40.0);
}

/// Loose fit does NOT force the child to fill: content smaller than the
/// slot shrink-wraps to its natural size (contrast `Expanded`, which
/// tightens to the slot).
#[test]
fn flexible_loose_fit_shrink_wraps_when_content_fits() {
    let (app, row_id, text_id) = setup_row_flex_item(
        r#"Flexible({ flex: 1 })
                .child(Text({ text: "Hi" }).fontSize(14).build())
                .build()"#,
    );

    let rt = app.element_tree();
    let row = rt.get_element(row_id).unwrap();
    let text = rt.get_element(text_id).unwrap();
    assert_eq!(row.computed_layout.size.width, 400.0);
    let w = text.computed_layout.size.width;
    assert!(
        w > 0.0 && w < 80.0,
        "content smaller than the 300 slot must shrink-wrap (natural width), got {w}"
    );
}

/// `Expanded` stays `FlexFit.tight`: the child is forced to fill its slot
/// even when its natural content is smaller.
#[test]
fn expanded_remains_tight_fill() {
    let (app, _row_id, text_id) = setup_row_flex_item(
        r#"Expanded({ flex: 1 })
                .child(Text({ text: "Hi" }).fontSize(14).build())
                .build()"#,
    );

    let rt = app.element_tree();
    let text = rt.get_element(text_id).unwrap();
    assert_eq!(
        text.computed_layout.size.width, 300.0,
        "Expanded tightens the child to exactly its slot"
    );
}

/// `Flexible().fit(FlexFit.Tight)` opts into Expanded semantics (Flutter's
/// `Flexible(fit: FlexFit.tight)` IS `Expanded`).
#[test]
fn flexible_fit_tight_behaves_like_expanded() {
    let (app, _row_id, text_id) = setup_row_flex_item(
        r#"Flexible({ flex: 1 })
                .fit(FlexFit.Tight)
                .child(Text({ text: "Hi" }).fontSize(14).build())
                .build()"#,
    );

    let rt = app.element_tree();
    let text = rt.get_element(text_id).unwrap();
    assert_eq!(
        text.computed_layout.size.width, 300.0,
        "FlexFit.Tight must force the child to fill its slot"
    );
}
