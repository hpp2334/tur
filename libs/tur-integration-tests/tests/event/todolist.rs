use tur_engine::core::element::ElementKind;
use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::InputElement;
use tur_engine::elements::TextSpanElement;
use tur_integration_tests::TurTestApp;

fn build_todolist() -> TurTestApp {
    let mut app = TurTestApp::new(800.0, 600.0).unwrap();
    app.load_bundle("todolist").unwrap();
    app
}

fn find_input_id(app: &TurTestApp) -> ElementNodeId {
    let wrapper_id = app.query_element(&["input-wrapper"]).expect("input-wrapper not found");
    let tree = app.element_tree();
    let wrapper = tree.get(wrapper_id).unwrap();
    let container = tree.get(wrapper.children[0]).unwrap();
    assert_eq!(
        container.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_container")
    );
    let input_node = tree.get(container.children[0]).unwrap();
    assert_eq!(
        input_node.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_input")
    );
    input_node.id
}

fn focus_input(app: &mut TurTestApp, input_id: ElementNodeId) {
    let (cx, cy) = app.get_element_absolute_bounds(input_id).unwrap().center();
    app.click(cx, cy);
}

fn todo_count(app: &TurTestApp) -> usize {
    let list_id = app.query_element(&["todo-list"]).expect("todo-list not found");
    let tree = app.element_tree();
    let list_node = tree.get(list_id).unwrap();
    list_node.children.len()
}

fn get_text_content(app: &TurTestApp, query_key: &[&str]) -> String {
    let id = app.query_element(query_key).unwrap_or_else(|| panic!("{:?} not found", query_key));
    let tree = app.element_tree();
    let container = tree.get(id).unwrap();
    let span_id = container.children.first().copied();
    drop(tree);
    span_id
        .and_then(|sid| {
            app.with_element(sid, |e| {
                e.cast::<TextSpanElement>()
                    .map(|s| s.content().to_string())
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default()
}

fn click_query_key(app: &mut TurTestApp, query_key: &[&str]) {
    app.render();
    let id = app.query_element(query_key).unwrap_or_else(|| panic!("{:?} not found", query_key));
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
    app.render();
}

fn get_selected_text(app: &TurTestApp) -> String {
    get_text_content(app, &["selected"])
}

fn click_todo_item(app: &mut TurTestApp, todo_id: i32) {
    let id = app.query_element(&["todo-item", &todo_id.to_string()]).unwrap();
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
    app.render();
}

fn get_input_text(app: &TurTestApp, input_id: ElementNodeId) -> String {
    app.with_element(input_id, |e| {
        e.cast::<InputElement>()
            .map(|i| i.text().to_string())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

#[test]
fn todolist_initial_render() {
    let mut app = build_todolist();
    app.render();

    assert_eq!(todo_count(&app), 4, "should start with 4 todos");
    assert_eq!(get_text_content(&app, &["toggle", "1"]), "\u{2713}");
    assert_eq!(get_text_content(&app, &["toggle", "2"]), "\u{25CB}");
    assert_eq!(get_text_content(&app, &["toggle", "3"]), "\u{25CB}");
    assert_eq!(get_text_content(&app, &["toggle", "4"]), "\u{25CB}");
}

#[test]
fn todolist_toggle_task() {
    let mut app = build_todolist();
    app.render();

    assert_eq!(get_text_content(&app, &["toggle", "1"]), "\u{2713}");

    click_query_key(&mut app, &["toggle", "1"]);

    let content = get_text_content(&app, &["toggle", "1"]);

    assert_eq!(
        content,
        "\u{25CB}",
        "toggled from done to undone"
    );
}

#[test]
fn todolist_toggle_multiple() {
    let mut app = build_todolist();
    app.render();

    click_query_key(&mut app, &["toggle", "1"]);
    click_query_key(&mut app, &["toggle", "2"]);
    click_query_key(&mut app, &["toggle", "3"]);

    assert_eq!(get_text_content(&app, &["toggle", "1"]), "\u{25CB}");
    assert_eq!(get_text_content(&app, &["toggle", "2"]), "\u{2713}");
    assert_eq!(get_text_content(&app, &["toggle", "3"]), "\u{2713}");
    assert_eq!(get_text_content(&app, &["toggle", "4"]), "\u{25CB}");
}

#[test]
fn todolist_input_typing() {
    let mut app = build_todolist();
    app.render();

    let input_id = find_input_id(&app);
    focus_input(&mut app, input_id);

    app.send_key("H");
    app.send_key("e");
    app.send_key("l");
    app.send_key("l");
    app.send_key("o");
    assert_eq!(get_input_text(&app, input_id), "Hello");
}

#[test]
fn todolist_input_enter_clears() {
    let mut app = build_todolist();
    app.render();

    let input_id = find_input_id(&app);
    focus_input(&mut app, input_id);

    app.send_key("T");
    app.send_key("e");
    app.send_key("s");
    app.send_key("t");
    assert_eq!(get_input_text(&app, input_id), "Test");

    app.send_key("Enter");
    assert_eq!(
        get_input_text(&app, input_id),
        "",
        "input should be cleared after Enter"
    );
}

#[test]
fn todolist_input_backspace() {
    let mut app = build_todolist();
    app.render();

    let input_id = find_input_id(&app);
    focus_input(&mut app, input_id);

    app.send_key("A");
    app.send_key("B");
    app.send_key("C");
    assert_eq!(get_input_text(&app, input_id), "ABC");

    app.send_key("Backspace");
    assert_eq!(get_input_text(&app, input_id), "AB");
}

#[test]
fn todolist_remove_click_fires() {
    let mut app = build_todolist();
    app.render();

    let remove_id = app.query_element(&["remove", "2"]).expect("remove-2 not found");
    let pi_id = {
        let tree = app.element_tree();
        let container = tree.get(remove_id).unwrap();
        container.parent.unwrap()
    };

    assert!(
        app.has_click_handler(pi_id),
        "remove button should have click handler"
    );

    click_query_key(&mut app, &["remove", "2"]);
}

#[test]
fn todolist_toggle_does_not_select() {
    let mut app = build_todolist();
    app.render();

    assert_eq!(get_selected_text(&app), "Selected: none");

    click_query_key(&mut app, &["toggle", "1"]);

    let content = get_text_content(&app, &["toggle", "1"]);
    assert_eq!(content, "\u{25CB}", "should have toggled");
    assert_eq!(get_selected_text(&app), "Selected: none", "clicking toggle should not select");
}

#[test]
fn todolist_click_item_selects() {
    let mut app = build_todolist();
    app.render();

    assert_eq!(get_selected_text(&app), "Selected: none");

    click_todo_item(&mut app, 2);

    assert_eq!(get_selected_text(&app), "Selected: 2");
}
