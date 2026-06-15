use std::time::Duration;

use tur_engine::core::element::ElementKind;
use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::Text;
use tur_integration_tests::TurTestApp;

fn get_text(app: &TurTestApp, qk: &[&str]) -> String {
    let id = app.query_element(qk).unwrap_or_else(|| panic!("{qk:?} not found"));
    app.with_element(id, |e| {
        e.cast::<Text>()
            .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn click_qk(app: &mut TurTestApp, qk: &[&str]) {
    let id = app.query_element(qk).unwrap_or_else(|| panic!("{qk:?} not found"));
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
}

fn find_input_id(app: &TurTestApp) -> ElementNodeId {
    let wrapper_id = app.query_element(&["edit-input"]).expect("edit-input not found");
    let tree = app.element_tree();
    let wrapper = tree.get(wrapper_id).unwrap();
    let inner = tree.get(wrapper.children[0]).unwrap();
    assert_eq!(
        inner.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_container")
    );
    let input_node = tree.get(inner.children[0]).unwrap();
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
    assert_eq!(get_text(&app, &["display"]), "Countdown: 60");
}

#[test]
fn countdown_start_tick() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-start"]);

    advance_seconds(&mut app, 1);

    assert_eq!(
        get_text(&app, &["display"]),
        "Countdown: 59",
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
        "Countdown: 55",
        "should be 55 after 5s"
    );
}

#[test]
fn countdown_pause() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-start"]);

    advance_seconds(&mut app, 1);
    assert_eq!(get_text(&app, &["display"]), "Countdown: 59");

    click_qk(&mut app, &["btn-pause"]);

    advance_seconds(&mut app, 3);

    assert_eq!(
        get_text(&app, &["display"]),
        "Countdown: 59",
        "should stay at 59 after pause"
    );
}

#[test]
fn countdown_reset() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-start"]);

    advance_seconds(&mut app, 5);
    assert_eq!(get_text(&app, &["display"]), "Countdown: 55");

    click_qk(&mut app, &["btn-reset"]);

    assert_eq!(
        get_text(&app, &["display"]),
        "Countdown: 60",
        "should reset to initial time"
    );
}

#[test]
fn countdown_edit_time() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-edit"]);

    let input_id = find_input_id(&app);
    focus_input(&mut app, input_id);

    app.send_key("3");
    app.send_key("0");

    click_qk(&mut app, &["btn-confirm"]);

    assert_eq!(
        get_text(&app, &["display"]),
        "Countdown: 30",
        "should update to edited time"
    );
}

#[test]
fn countdown_edit_then_start() {
    let mut app = build_countdown();
    click_qk(&mut app, &["btn-edit"]);

    let input_id = find_input_id(&app);
    focus_input(&mut app, input_id);

    app.send_key("1");
    app.send_key("0");

    click_qk(&mut app, &["btn-confirm"]);

    click_qk(&mut app, &["btn-start"]);

    advance_seconds(&mut app, 1);

    assert_eq!(
        get_text(&app, &["display"]),
        "Countdown: 9",
        "should count down from edited time"
    );
}
