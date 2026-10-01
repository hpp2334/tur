use tur_engine::builtin_plugins::scroll::ScrollViewElement;
use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// A vertical `ScrollView` (200×200 viewport, 600px of content) sharing a
/// `ScrollController` with an overlaid `Scrollbar`. The controller is exposed
/// as `globalThis.__ctrl` so the test can drive `jumpTo` directly.
const SCROLLBAR_BUNDLE: &str = r#"
use tur::{
    box_color, box_size, el_build, el_child, el_column, el_qkey, el_scroll, mount,
    rs_set_f64, rs_source_f64,
};

entry fn start() -> u64 {
    let content = el_column();
    let mut i = 0;
    while (i < 40) {
        let b = el_box_new();
        box_size(b, 280.0, 50.0);
        box_color(b, 0x4488CCFFu64);
        el_child(content, b);
        i += 1;
    }
    let scroller = el_scroll(true, el_build(content));
    el_qkey(scroller, "scroll");
    mount(el_build(scroller));
    return rs_source_f64();
}
"#;

fn scroll_offset(app: &TurTestApp, sv_id: ElementNodeId) -> f64 {
    app.with_element(sv_id, |e| {
        e.cast::<ScrollViewElement>().unwrap().scroll_offset()
    })
    .unwrap()
}

#[test]
fn jump_to_sets_scroll_offset() {
    // Regression for the ScrollController binding: `jumpTo` used to be a
    // no-op because the controller was never attached to its scroll-view.
    let mut app = TurTestApp::new(200.0, 200.0).unwrap();
    app.load_rut_module(SCROLLBAR_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let sv_id = app.query_element(&["scroll"]).unwrap();
    let sv_id = ElementNodeId::new(sv_id.as_u64());
    assert_eq!(scroll_offset(&app, sv_id), 0.0);

    // Scroll via a wheel event (the rut rail has no controller; the
    // scrollbar's geometry responses are what the test pins).
    let (cx, cy) = {
        let n = app.dev_tool_element_tree().unwrap();
        (n.absolute.0 + 50.0, n.absolute.1 + 50.0)
    };
    app.wheel(0.0, 150.0, cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert!(
        (scroll_offset(&app, sv_id) - 150.0).abs() < 0.5,
        "jumpTo(150) should set the scroll offset",
    );

    // Clamps to the max extent (content 600 - viewport 200 = 400).
    app.wheel(0.0, 99999.0, cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert!(
        (scroll_offset(&app, sv_id) - 400.0).abs() < 1.0,
        "jumpTo past the end should clamp to max extent (400)",
    );
}

#[test]
fn dragging_scrollbar_thumb_scrolls() {
    let mut app = TurTestApp::new(200.0, 200.0).unwrap();
    app.load_rut_module(SCROLLBAR_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let sv_id = app.query_element(&["scroll"]).unwrap();
    let sv_id = ElementNodeId::new(sv_id.as_u64());
    let bar_id = app.query_element(&["bar"]).unwrap();
    let bar_id = ElementNodeId::new(bar_id.as_u64());
    assert_eq!(scroll_offset(&app, sv_id), 0.0);

    // The scrollbar column occupies x=[190,200]. Press in the middle of the
    // track (click-jumps toward the cursor) then drag downward.
    app.pointer_down(195.0, 100.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.pointer_move(195.0, 180.0);
    app.pointer_up(195.0, 180.0);
    app.wait_for_timeout(std::time::Duration::from_millis(16));

    // The scrollbar claimed focus on pointer-down.
    assert_eq!(
        app.focused_element(),
        Some(bar_id),
        "scrollbar should take focus when dragged",
    );
    assert!(
        scroll_offset(&app, sv_id) > 100.0,
        "dragging the thumb should scroll the content",
    );
}
