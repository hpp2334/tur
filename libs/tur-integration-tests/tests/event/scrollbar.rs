use tur_engine::builtin_plugins::scroll::ScrollViewElement;
use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// A vertical `ScrollView` (200×200 viewport, 600px of content) sharing a
/// `ScrollController` with an overlaid `Scrollbar`. The controller is exposed
/// as `globalThis.__ctrl` so the test can drive `jumpTo` directly.
const SCROLLBAR_BUNDLE: &str = r#"
import { setViewRoot, viewRoot, Container, Row, Expanded, ScrollView, Column, Scrollbar } from "tur:std";
globalThis.__ctrl = new globalThis.ScrollController();
const blocks = [];
for (let i = 0; i < 6; i++) blocks.push(Container({ height: 100 }));
setViewRoot(viewRoot("main"), Row({
    children: [
        Expanded({
            child: ScrollView({
                controller: globalThis.__ctrl,
                queryKey: ["scroll"],
                child: Column({ children: blocks }),
            }),
        }),
        Scrollbar({
            controller: globalThis.__ctrl,
            thickness: 10,
            queryKey: ["bar"],
        }),
    ],
}));
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
    let app = TurTestApp::new(200.0, 200.0).unwrap();
    app.eval_module_source(SCROLLBAR_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let sv_id = app.query_element(&["scroll"]).unwrap();
    let sv_id = sv_id.as_element_id();
    assert_eq!(scroll_offset(&app, sv_id), 0.0);

    app.eval_js("globalThis.__ctrl.jumpTo(150)");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert!(
        (scroll_offset(&app, sv_id) - 150.0).abs() < 0.5,
        "jumpTo(150) should set the scroll offset",
    );

    // Clamps to the max extent (content 600 - viewport 200 = 400).
    app.eval_js("globalThis.__ctrl.jumpTo(99999)");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert!(
        (scroll_offset(&app, sv_id) - 400.0).abs() < 1.0,
        "jumpTo past the end should clamp to max extent (400)",
    );
}

#[test]
fn dragging_scrollbar_thumb_scrolls() {
    let mut app = TurTestApp::new(200.0, 200.0).unwrap();
    app.eval_module_source(SCROLLBAR_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let sv_id = app.query_element(&["scroll"]).unwrap();
    let sv_id = sv_id.as_element_id();
    let bar_id = app.query_element(&["bar"]).unwrap();
    let bar_id = bar_id.as_element_id();
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
