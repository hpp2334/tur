use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

fn qid(app: &TurTestApp, key: &[&str]) -> ElementNodeId {
    let id = app.query_element(key).expect("queryKey not found");
    ElementNodeId::new(id.as_u64())
}

/// Regression for the github-viewer crash: a pointer-up that lands outside any
/// focusable element while an `Input` is focused used to panic with "RefCell
/// already borrowed" inside `GestureSubsystem::handle_mouse_pointer_up`. A
/// `let`-chain condition (`let Some(focused) = cx.focus_manager.borrow().focused()`)
/// kept the immutable `focus_manager` borrow alive across the `borrow_mut()`
/// that clears focus, because `let`-chain temporaries have their lifetimes
/// extended through the whole `if` block.
#[test]
fn pointer_up_outside_focusable_clears_focus_without_panic() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("focus-clear-on-click").unwrap();

    let editable_id = qid(&app, &["editable"]);

    app.render();
    assert!(
        app.focused_element().is_none(),
        "nothing should be focused before any click"
    );

    // Focus the Input by clicking inside it.
    let (ex, ey) = app
        .get_element_absolute_bounds(editable_id)
        .unwrap()
        .center();
    app.click(ex, ey);
    assert!(
        app.focused_element().is_some(),
        "Input should be focused after clicking it"
    );

    // Click dead space (outside the Input and the button, inside the Column's
    // empty area): focusable_id=None + focused=Some + down_target != focused —
    // the exact condition that used to reach `borrow_mut()` and panic. Without
    // the fix this aborts with "RefCell already borrowed".
    app.click(200.0, 300.0);

    assert!(
        app.focused_element().is_none(),
        "focus should be cleared after a pointer-up outside any focusable"
    );
}
