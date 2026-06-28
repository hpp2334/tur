use std::time::Duration;

use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::elements::TextElement;
use tur_integration_tests::TurTestApp;

fn get_text(app: &TurTestApp, qk: &[&str]) -> String {
    let id = app.query_element(qk).unwrap_or_else(|| panic!("{qk:?} not found"));
    let id = ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<TextElement>()
            .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn click_qk(app: &mut TurTestApp, qk: &[&str]) {
    let id = app.query_element(qk).unwrap_or_else(|| panic!("{qk:?} not found"));
    let id = ElementNodeId::new(id.as_u64());
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
}

fn find_input_id(app: &TurTestApp) -> ElementNodeId {
    let wrapper_id = app.query_element(&["edit-input"]).expect("edit-input not found");
    let wrapper_id = ElementNodeId::new(wrapper_id.as_u64());
    let tree = app.element_tree();
    let wrapper = tree.get_element(wrapper_id).unwrap();
    let inner = tree.get_element(ElementNodeId::new(wrapper.children[0].as_u64())).unwrap();
    assert_eq!(
        inner.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_container")
    );
    let input_node = tree.get_element(ElementNodeId::new(inner.children[0].as_u64())).unwrap();
    assert_eq!(
        input_node.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_editable_text")
    );
    input_node.id
}

fn focus_input(app: &mut TurTestApp, input_id: ElementNodeId) {
    let (cx, cy) = app.get_element_absolute_bounds(input_id).unwrap().center();
    app.click(cx, cy);
}

fn build_countdown() -> TurTestApp {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("countdown").unwrap();
    app
}

fn advance_seconds(app: &mut TurTestApp, secs: u32) {
    for _ in 0..secs {
        app.advance(Duration::from_secs(1)).unwrap();
        app.render();
    }
}

#[test]
fn countdown_initial_render() {
    let app = build_countdown();
    assert_eq!(get_text(&app, &["display"]), "1:00");
}

#[test]
fn countdown_start_tick() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-start"]);

    advance_seconds(&mut app, 1);

    assert_eq!(
        get_text(&app, &["display"]),
        "0:59",
        "should decrement after 1s"
    );
}

#[test]
fn countdown_start_multiple_ticks() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-start"]);

    advance_seconds(&mut app, 5);

    assert_eq!(
        get_text(&app, &["display"]),
        "0:55",
        "should be 0:55 after 5s"
    );
}

#[test]
fn countdown_pause() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-start"]);

    advance_seconds(&mut app, 1);
    assert_eq!(get_text(&app, &["display"]), "0:59");

    click_qk(&mut app, &["btn-pause"]);

    advance_seconds(&mut app, 3);

    assert_eq!(
        get_text(&app, &["display"]),
        "0:59",
        "should stay at 0:59 after pause"
    );
}

#[test]
fn countdown_reset() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-start"]);

    advance_seconds(&mut app, 5);
    assert_eq!(get_text(&app, &["display"]), "0:55");

    click_qk(&mut app, &["btn-reset"]);

    assert_eq!(
        get_text(&app, &["display"]),
        "1:00",
        "should reset to initial time"
    );
}

#[test]
fn countdown_edit_time() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-edit"]);

    let input_id = find_input_id(&app);
    focus_input(&mut app, input_id);

    // The modal pre-fills the field with the current value (60); clear it
    // before typing the new value.
    app.send_key_with_modifiers_full("a", false, true, true);
    app.send_key("Backspace");
    app.send_key("3");
    app.send_key("0");

    click_qk(&mut app, &["btn-confirm"]);

    assert_eq!(
        get_text(&app, &["display"]),
        "0:30",
        "should update to edited time"
    );
}

#[test]
fn countdown_edit_then_start() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-edit"]);

    let input_id = find_input_id(&app);
    focus_input(&mut app, input_id);

    // Clear the pre-filled "60" before typing.
    app.send_key_with_modifiers_full("a", false, true, true);
    app.send_key("Backspace");
    app.send_key("1");
    app.send_key("0");

    click_qk(&mut app, &["btn-confirm"]);

    click_qk(&mut app, &["btn-start"]);

    advance_seconds(&mut app, 1);

    assert_eq!(
        get_text(&app, &["display"]),
        "0:09",
        "should count down from edited time"
    );
}
