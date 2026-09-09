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
            ElementKind::new("tur_flexible"),
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

// ===========================================================================
// Regression: Flexible inside a MainAxisSize.min Row — the pill shape.
//
// Under an UNBOUNDED incoming main axis (the pill Row is a non-flex child of
// a bounded Row, which passes unbounded main down — RenderFlex parity in
// both engines), Flutter's RenderFlex treats flex children as INFLEXIBLE
// (`canFlex == maxMainSize.isFinite`): they lay out with unbounded main
// constraints and their actual sizes feed the MainAxisSize.min sum. tur
// collapsed their slot to zero instead — the label's Text then ran its
// internal layout unbounded (painting natural-width glyphs from a
// zero-wide box: caret-on-top-of-glyphs) while siblings were positioned per
// the zero slot (chrome too narrow, "pill 40% narrower").
// ===========================================================================

/// The reporter's pill: `Row(mainAxisSize: Min)` as a non-flex child of a
/// bounded Row → unbounded main. The label must lay out at its NATURAL
/// width (Flutter: inflexible under unbounded), the pill must shrink-wrap
/// to Σ actual child sizes, and the caret must sit 8dp after the label's
/// right edge — never on top of its glyphs.
#[test]
fn flexible_min_size_row_under_unbounded_main_shrink_wraps() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.eval_module_source(
        r#"
        import {
            mount, Row, Flexible, Text, SizedBox, MainAxisSize,
        } from "tur:std";
        mount(Row()
            .children([
                Row().mainAxisSize(MainAxisSize.Min).queryKey(["pill"]).children([
                    SizedBox().width(14).build(),
                    Flexible({ flex: 1 })
                        .queryKey(["label"])
                        .child(Text({ text: "Last Week Todos" })
                            .fontSize(14)
                            .maxLines(1)
                            .overflow("ellipsis")
                            .build())
                        .build(),
                    SizedBox().width(8).build(),
                    Text({ text: "v" }).fontSize(14).queryKey(["caret"]).build(),
                    SizedBox().width(14).build(),
                ]).build(),
            ])
            .build());
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let ids: Vec<tur_engine::core::element::NodeId> = ["pill", "label", "caret"]
        .iter()
        .map(|k| app.query_element(&[k]).unwrap())
        .collect();
    let rt = app.element_tree();
    let pill = rt.get_element(ElementNodeId::new(ids[0].as_u64())).unwrap();
    let label = rt.get_element(ElementNodeId::new(ids[1].as_u64())).unwrap();
    let caret = rt.get_element(ElementNodeId::new(ids[2].as_u64())).unwrap();

    let label_w = label.computed_layout.size.width;
    let caret_w = caret.computed_layout.size.width;
    assert!(
        label_w > 60.0 && label_w < 200.0,
        "under an unbounded main axis the label must lay out at its natural \
         width (Flutter treats the flex child as inflexible), got {label_w} \
         — a collapsed slot truncates it to the ellipsis"
    );
    assert_eq!(
        label.children.len(),
        1,
        "the Flexible keeps its child in the tree"
    );

    // The painted text never exceeds its layout box (no glyphs under the
    // caret) — read via `trace_layout_extra`'s `layoutWidth` (the parley
    // layout width, i.e. the PAINT extent, distinct from the clamped SIZE).
    let text_id = ElementNodeId::new(label.children[0].as_u64());
    let painted = app
        .with_element(text_id, |e| {
            use tur_engine::builtin_plugins::text::TextElement;
            use tur_engine::core::elements::{ElementTrace, TraceValue};
            e.cast::<TextElement>()
                .unwrap()
                .trace_layout_extra()
                .into_iter()
                .find(|(k, _)| *k == "layoutWidth")
                .map(|(_, v)| match v {
                    TraceValue::Num(n) => n,
                    _ => 0.0,
                })
                .unwrap_or(0.0)
        })
        .unwrap();
    assert!(
        painted <= label_w + 0.5,
        "the painted text must fit its layout box: painted={painted}, box={label_w}"
    );

    // Caret sits exactly 8dp after the label's right edge.
    let gap = caret.computed_layout.offset.x - (label.computed_layout.offset.x + label_w);
    assert!(
        (gap - 8.0).abs() < 0.5,
        "caret must sit 8dp after the label (gap={gap}), not on top of its glyphs"
    );

    // The pill shrink-wraps to Σ actual child sizes.
    let expected_pill_w = 14.0 + label_w + 8.0 + caret_w + 14.0;
    assert!(
        (pill.computed_layout.size.width - expected_pill_w).abs() < 0.5,
        "MainAxisSize.min Row must size to Σ actual child sizes: expected \
         {expected_pill_w}, got {}",
        pill.computed_layout.size.width
    );
}

/// The bounded variant (the reporter's requested repro): in a bounded
/// min-size Row the Flexible slot IS the remaining space — a long label
/// ellipsizes at it, and the Row still shrink-wraps when the label is short.
#[test]
fn min_size_row_flexible_uses_remaining_budget_and_shrink_wraps() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.eval_module_source(
        r#"
        import {
            mount, Column, Row, Container, Flexible, Text, SizedBox, MainAxisSize,
        } from "tur:std";
        mount(Column()
            .children([
                Row().mainAxisSize(MainAxisSize.Min).queryKey(["long-row"]).children([
                    Container().width(100).height(40).build(),
                    Flexible({ flex: 1 })
                        .child(Text({
                            text: "A very long label that must ellipsize inside its slot",
                        })
                            .fontSize(14)
                            .maxLines(1)
                            .overflow("ellipsis")
                            .build())
                        .build(),
                    SizedBox().width(20).build(),
                ]).build(),
                Row().mainAxisSize(MainAxisSize.Min).queryKey(["short-row"]).children([
                    Container().width(100).height(40).build(),
                    Flexible({ flex: 1 })
                        .child(Text({ text: "Hi" }).fontSize(14).build())
                        .build(),
                    SizedBox().width(20).build(),
                ]).build(),
            ])
            .build());
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let long_row = ElementNodeId::new(app.query_element(&["long-row"]).unwrap().as_u64());
    let short_row = ElementNodeId::new(app.query_element(&["short-row"]).unwrap().as_u64());
    let rt = app.element_tree();
    let long_row_node = rt.get_element(long_row).unwrap();
    let short_row_node = rt.get_element(short_row).unwrap();

    // Long label: budget = 400 − 100 − 20 = 280 → ellipsize near it; the
    // min-size Row then sizes to Σ actuals (≈ 400, not forced past it).
    let long_text = rt
        .get_element(ElementNodeId::new(long_row_node.children[1].as_u64()))
        .unwrap();
    let long_text = rt
        .get_element(ElementNodeId::new(long_text.children[0].as_u64()))
        .unwrap();
    let w = long_text.computed_layout.size.width;
    assert!(
        w >= 250.0 && w <= 280.0,
        "long label must use the remaining budget (250–280), got {w}"
    );
    let long_row_w = long_row_node.computed_layout.size.width;
    assert!(
        (long_row_w - (100.0 + w + 20.0)).abs() < 1.0,
        "min-size Row must size to Σ actual child sizes: got {long_row_w}"
    );

    // Short label: shrink-wraps — the Row is far narrower than the 400 max.
    let short_text = rt
        .get_element(ElementNodeId::new(short_row_node.children[1].as_u64()))
        .unwrap();
    let short_text = rt
        .get_element(ElementNodeId::new(short_text.children[0].as_u64()))
        .unwrap();
    let sw = short_text.computed_layout.size.width;
    assert!(sw > 0.0 && sw < 60.0, "short label shrink-wraps, got {sw}");
    let short_row_w = short_row_node.computed_layout.size.width;
    assert!(
        (short_row_w - (100.0 + sw + 20.0)).abs() < 1.0 && short_row_w < 200.0,
        "min-size Row with short content must be compact: got {short_row_w}"
    );
}

/// When the non-flex siblings fill the row exactly, the remaining space is
/// zero — the loose slot is `min: 0, max: 0`, and the Text's INTERNAL
/// (painted) layout must respect that zero budget: at most the ellipsis
/// glyph, never its natural width (the zero-wide box would otherwise paint
/// full-width glyphs under the sibling content).
#[test]
fn flexible_zero_remaining_slot_paints_within_budget() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.eval_module_source(
        r#"
        import { mount, Row, Container, Flexible, Text } from "tur:std";
        mount(Row()
            .children([
                Container().width(400).height(40).build(),
                Flexible({ flex: 1 })
                    .queryKey(["label"])
                    .child(Text({
                        text: "A very long label that must not paint naturally here",
                    })
                        .fontSize(14)
                        .maxLines(1)
                        .overflow("ellipsis")
                        .build())
                    .build(),
            ])
            .build());
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let label = ElementNodeId::new(app.query_element(&["label"]).unwrap().as_u64());
    let text_id = {
        let rt = app.element_tree();
        let label_node = rt.get_element(label).unwrap();
        let text_node = rt
            .get_element(ElementNodeId::new(label_node.children[0].as_u64()))
            .unwrap();
        text_node.id
    };
    let size_w = {
        let rt = app.element_tree();
        rt.get_element(text_id).unwrap().computed_layout.size.width
    };
    let painted = app
        .with_element(text_id, |e| {
            use tur_engine::builtin_plugins::text::TextElement;
            use tur_engine::core::elements::{ElementTrace, TraceValue};
            e.cast::<TextElement>()
                .unwrap()
                .trace_layout_extra()
                .into_iter()
                .find(|(k, _)| *k == "layoutWidth")
                .map(|(_, v)| match v {
                    TraceValue::Num(n) => n,
                    _ => 0.0,
                })
                .unwrap_or(0.0)
        })
        .unwrap();
    assert_eq!(
        size_w, 0.0,
        "the zero remaining slot collapses the computed size to 0"
    );
    assert!(
        painted <= 15.0,
        "the internal (painted) layout must respect the zero budget — at \
         most the ellipsis glyph (~1em ≈ 14dp at fontSize 14), got {painted}"
    );
}
