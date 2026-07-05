use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::ScrollViewElement;
use tur_integration_tests::TurTestApp;

/// A multiline editor inside a ScrollView, pre-filled with 30 lines so the
/// content (~504px at 14px/1.2 line-height) is far taller than the 200px
/// viewport. The ScrollView is the root element, so it receives the window
/// size as a bounded viewport.
const CARET_SCROLL_BUNDLE: &str = r#"
import { render, ScrollView, InputEdgy } from "builtin:tur/std";
const lines = [];
for (let i = 0; i < 30; i++) lines.push("line " + i);
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans([{ content: lines.join("\n") }]);
render(ScrollView({
    queryKey: ["scroll"],
    child: InputEdgy({
        controller: globalThis.__ctrl,
        multiline: true,
        fontSize: 14,
        fontFamily: "monospace",
        queryKey: ["editor"],
    }),
}));
"#;

fn scroll_offset(app: &TurTestApp, sv_id: ElementNodeId) -> f64 {
    app.with_element(sv_id, |e| {
        e.cast::<ScrollViewElement>().unwrap().scroll_offset()
    })
    .unwrap()
}

#[test]
fn caret_into_view_scrolls_to_caret() {
    let mut app = TurTestApp::new(300.0, 200.0).unwrap();
    app.eval_module_source(CARET_SCROLL_BUNDLE).unwrap();
    app.render();

    let sv_id = app.query_element(&["scroll"]).unwrap();
    let sv_id = ElementNodeId::new(sv_id.as_u64());
    assert_eq!(scroll_offset(&app, sv_id), 0.0, "no scroll before caret moves");

    // Focus the editor near its top-left (the editable occupies the top of the
    // scroll content at offset 0).
    app.click(5.0, 8.0);
    app.render();
    assert!(app.focused_element().is_some(), "editor should be focused after click");

    // Walk the caret down to the last line. Every keydown also runs
    // ensure_caret_visible, which scrolls the viewport to follow the caret
    // once it leaves the visible region.
    for _ in 0..35 {
        app.send_key("ArrowDown");
    }
    app.render();

    let after_down = scroll_offset(&app, sv_id);
    assert!(
        after_down > 200.0,
        "viewport should scroll down to follow the caret (got offset={after_down})",
    );

    // Walking the caret back to the top must scroll the viewport back up.
    for _ in 0..35 {
        app.send_key("ArrowUp");
    }
    app.render();

    let after_up = scroll_offset(&app, sv_id);
    assert!(
        after_up < after_down - 100.0,
        "viewport should scroll back up after ArrowUp (got {after_up}, was {after_down})",
    );
    assert!(
        after_up < 50.0,
        "viewport should be near the top after returning the caret to line 0 (got {after_up})",
    );
}
