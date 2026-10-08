use tur_engine::builtin_plugins::scroll::ScrollViewElement;
use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// A multiline editor inside a ScrollView, pre-filled with 30 lines so the
/// content (~504px at 14px/1.2 line-height) is far taller than the 200px
/// viewport. The ScrollView is the root element, so it receives the window
/// size as a bounded viewport.
const CARET_SCROLL_BUNDLE: &str = r#"
use tur_host::{ AXIS_VERTICAL, tctrl_new, tctrl_push_span, undo_new };
use tur_kit::{ Column, Input, ScrollView, TextCtrl, UndoCtrl, mount };


entry fn start() {
    let ctrl = tctrl_new();
    let mut i = 0;
    while (i < 30) {
        tctrl_push_span(ctrl, f"line {i}\n");
        i += 1;
    }
    let undo = undo_new();
    // Multiline (flags bit 0), auto height — the editable lays out at its
    // content height (~30 lines) so the ScrollView has overflow to scroll.
    // The width spans the window (the JS twin's stretched-column geometry):
    // a ScrollView shrink-wraps its cross axis, so without it the whole
    // scroller would hug the longest line and the top-left click misses it.
    let mut input = Input().controller(TextCtrl(ctrl)).undo(UndoCtrl(undo)).width_height(300.0, 0.0).multiline(true).query_key("editor").build();
    let input = input;
    let col = Column().child(input);
    let mut scroller = ScrollView().axis(AXIS_VERTICAL).child(col.build()).query_key("scroll").build();
    let scroller = scroller;
    mount(scroller);
}
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
    app.load_rut_module(CARET_SCROLL_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let sv_id = app.query_element(&["scroll"]).unwrap();
    let sv_id = ElementNodeId::new(sv_id.as_u64());
    assert_eq!(
        scroll_offset(&app, sv_id),
        0.0,
        "no scroll before caret moves"
    );

    // Focus the editor near its top-left (the editable occupies the top of the
    // scroll content at offset 0).
    app.click(5.0, 8.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert!(
        app.focused_element().is_some(),
        "editor should be focused after click"
    );

    // Walk the caret down to the last line. Every keydown also runs
    // ensure_caret_visible, which scrolls the viewport to follow the caret
    // once it leaves the visible region.
    for _ in 0..35 {
        app.send_key("ArrowDown");
        app.wait_for_timeout(std::time::Duration::ZERO);
    }
    app.wait_for_timeout(std::time::Duration::ZERO);

    let after_down = scroll_offset(&app, sv_id);
    assert!(
        after_down > 200.0,
        "viewport should scroll down to follow the caret (got offset={after_down})",
    );

    // Walking the caret back to the top must scroll the viewport back up.
    for _ in 0..35 {
        app.send_key("ArrowUp");
        app.wait_for_timeout(std::time::Duration::ZERO);
    }
    app.wait_for_timeout(std::time::Duration::ZERO);

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

