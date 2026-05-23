use tur_engine::core::element::ElementKind;
use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::InputElement;
use tur_engine::elements::TextSpanElement;
use tur_integration_tests::TurTestApp;

fn build_app() -> TurTestApp {
    let mut app = TurTestApp::new(800.0, 600.0).unwrap();
    app.load_bundle("input-jotai").unwrap();
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

fn get_input_text(app: &TurTestApp, input_id: ElementNodeId) -> String {
    app.with_element(input_id, |e| {
        e.cast::<InputElement>()
            .map(|i| i.text().to_string())
            .unwrap_or_default()
    })
    .unwrap_or_default()
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

fn read_jotai_atom(app: &mut TurTestApp) -> String {
    app.eval_js("globalThis.__jotaiTestStore.get(globalThis.__jotaiInputText)")
}

#[test]
fn input_on_input_updates_jotai_atom() {
    let mut app = build_app();
    app.render();

    assert_eq!(read_jotai_atom(&mut app), "", "atom should start empty");

    let input_id = find_input_id(&app);
    focus_input(&mut app, input_id);

    app.send_key("H");
    assert_eq!(get_input_text(&app, input_id), "H");
    assert_eq!(read_jotai_atom(&mut app), "H", "atom should update after typing");

    app.send_key("i");
    assert_eq!(get_input_text(&app, input_id), "Hi");
    assert_eq!(read_jotai_atom(&mut app), "Hi", "atom should update after second key");
}

#[test]
fn button_state_updates_after_typing() {
    let mut app = build_app();
    app.render();

    for _ in 0..10 {
        let _ = app.tick();
    }

    assert_eq!(get_text_content(&app, &["button-text"]), "Disabled");
    assert_eq!(get_text_content(&app, &["debug-text"]), "text:\"\"");

    let input_id = find_input_id(&app);
    focus_input(&mut app, input_id);
    app.send_key("A");

    assert_eq!(read_jotai_atom(&mut app), "A");
    assert_eq!(get_text_content(&app, &["button-text"]), "Active", "button should be active after typing");
    assert_eq!(get_text_content(&app, &["debug-text"]), "text:\"A\"");
}
