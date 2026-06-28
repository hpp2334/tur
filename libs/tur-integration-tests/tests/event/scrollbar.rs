use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::ScrollViewElement;
use tur_integration_tests::TurTestApp;

/// A vertical `ScrollView` (200×200 viewport, 600px of content) sharing a
/// `ScrollController` with an overlaid `Scrollbar`. The controller is exposed
/// as `globalThis.__ctrl` so the test can drive `jumpTo` directly.
const SCROLLBAR_BUNDLE: &str = r#"
const T = globalThis.__tur;
const ctx = T.__ctx;
globalThis.__ctrl = new globalThis.ScrollController();
const blocks = [];
for (let i = 0; i < 6; i++) blocks.push(T.Container(ctx, { height: 100 }));
T.render(ctx, T.Row(ctx, {
    children: [
        T.Expanded(ctx, {
            child: T.ScrollView(ctx, {
                controller: globalThis.__ctrl,
                queryKey: ["scroll"],
                child: T.Column(ctx, { children: blocks }),
            }),
        }),
        T.Scrollbar(ctx, {
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
    let mut app = TurTestApp::new(200.0, 200.0).unwrap();
    app.load_bundle_source(SCROLLBAR_BUNDLE).unwrap();
    app.render();

    let sv_id = app.query_element(&["scroll"]).unwrap();
    let sv_id = ElementNodeId::new(sv_id.as_u64());
    assert_eq!(scroll_offset(&app, sv_id), 0.0);

    app.eval_js("globalThis.__ctrl.jumpTo(150)");
    app.render();
    assert!(
        (scroll_offset(&app, sv_id) - 150.0).abs() < 0.5,
        "jumpTo(150) should set the scroll offset",
    );

    // Clamps to the max extent (content 600 - viewport 200 = 400).
    app.eval_js("globalThis.__ctrl.jumpTo(99999)");
    app.render();
    assert!(
        (scroll_offset(&app, sv_id) - 400.0).abs() < 1.0,
        "jumpTo past the end should clamp to max extent (400)",
    );
}

#[test]
fn dragging_scrollbar_thumb_scrolls() {
    let mut app = TurTestApp::new(200.0, 200.0).unwrap();
    app.load_bundle_source(SCROLLBAR_BUNDLE).unwrap();
    app.render();

    let sv_id = app.query_element(&["scroll"]).unwrap();
    let sv_id = ElementNodeId::new(sv_id.as_u64());
    let bar_id = app.query_element(&["bar"]).unwrap();
    let bar_id = ElementNodeId::new(bar_id.as_u64());
    assert_eq!(scroll_offset(&app, sv_id), 0.0);

    // The scrollbar column occupies x=[190,200]. Press in the middle of the
    // track (click-jumps toward the cursor) then drag downward.
    app.pointer_down_no_flush(195.0, 100.0);
    app.pointer_move_no_flush(195.0, 180.0);
    app.pointer_up_no_flush(195.0, 180.0);
    app.tick().unwrap();
    app.render();

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
