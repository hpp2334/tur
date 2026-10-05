//! The `todolist` corpus case (the phase-8 rewrite): the boa Tasks board —
//! toggle / add / remove through the REAL interaction paths (checkbox taps,
//! the add modal, the remove-confirm modal). No probes: every journey is
//! driven through clicks on query keys, and state reads back through bound
//! texts.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

fn build() -> TurTestApp {
    let mut app = TurTestApp::new(430.0, 760.0).unwrap();
    app.load_rut_bundle("todolist").unwrap();
    app.wait_for_timeout(Duration::ZERO);
    app
}

fn click_qk(app: &mut TurTestApp, qk: &[&str]) {
    use tur_engine::core::element::ElementNodeId;
    let id = app
        .query_element(qk)
        .unwrap_or_else(|| panic!("{qk:?} not found"));
    let (cx, cy) = app
        .get_element_absolute_bounds(ElementNodeId::new(id.as_u64()))
        .unwrap()
        .center();
    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
}

/// The first todo-check qk = task 0 ("Buy groceries", open).
#[test]
fn todolist_case_boots_the_tasks_board() {
    let app = build();
    assert_eq!(
        app.query_text(&["todo-count"]).as_deref(),
        Some("5 items · 1 done"),
        "the derived counter over the seed list"
    );
    assert!(
        app.query_element(&["todo-add-open"]).is_some(),
        "the New Task pill"
    );
    assert!(
        app.query_element(&["todo-delete"]).is_some(),
        "the X buttons"
    );
    // Neither modal boots open (the boa boot-modal bug is not ported).
    assert!(
        app.query_element(&["todo-add-title"]).is_none(),
        "the add modal is closed at boot"
    );
    assert!(
        app.query_element(&["todo-rm-msg"]).is_none(),
        "the remove modal is closed at boot"
    );
}

#[test]
fn todolist_case_toggles_done_through_the_checkbox() {
    let mut app = build();
    // Task 0 is open; the toggle flips the derive to 2 done.
    click_qk(&mut app, &["todo-check"]);
    assert_eq!(
        app.query_text(&["todo-count"]).as_deref(),
        Some("5 items · 2 done"),
        "the checkbox tap flips the task's done flag"
    );
    // Toggle back.
    click_qk(&mut app, &["todo-check"]);
    assert_eq!(
        app.query_text(&["todo-count"]).as_deref(),
        Some("5 items · 1 done")
    );
}

#[test]
fn todolist_case_adds_through_the_modal() {
    let mut app = build();
    click_qk(&mut app, &["todo-add-open"]);
    assert!(
        app.query_element(&["todo-add-title"]).is_some(),
        "the add modal opened"
    );
    // Cancel first — the modal closes without appending.
    click_qk(&mut app, &["todo-add-cancel"]);
    assert!(app.query_element(&["todo-add-title"]).is_none());
    assert_eq!(
        app.query_text(&["todo-count"]).as_deref(),
        Some("5 items · 1 done")
    );

    // Reopen, type a title, submit.
    click_qk(&mut app, &["todo-add-open"]);
    click_qk(&mut app, &["todo-add-title"]);
    app.send_key("h");
    app.send_key("i");
    app.wait_for_timeout(Duration::ZERO);
    click_qk(&mut app, &["todo-add-submit"]);
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["todo-add-title"]).is_none(),
        "the modal closed on submit"
    );
    assert_eq!(
        app.query_text(&["todo-count"]).as_deref(),
        Some("6 items · 1 done"),
        "the fresh task appended open"
    );
}

#[test]
fn todolist_case_removes_through_the_confirm_modal() {
    let mut app = build();
    // X on task 0: the confirm modal names the targeted task (built at
    // open time via then_build).
    click_qk(&mut app, &["todo-delete"]);
    let msg = app
        .query_text(&["todo-rm-msg"])
        .expect("the remove modal opened");
    assert!(
        msg.contains("Buy groceries"),
        "the modal names the targeted task: {msg:?}"
    );
    // Cancel keeps the list.
    click_qk(&mut app, &["todo-rm-cancel"]);
    assert!(app.query_element(&["todo-rm-msg"]).is_none());
    assert_eq!(
        app.query_text(&["todo-count"]).as_deref(),
        Some("5 items · 1 done")
    );

    // Remove for real.
    click_qk(&mut app, &["todo-delete"]);
    click_qk(&mut app, &["todo-rm-confirm"]);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.query_text(&["todo-count"]).as_deref(),
        Some("4 items · 1 done"),
        "the task left the list"
    );
}
