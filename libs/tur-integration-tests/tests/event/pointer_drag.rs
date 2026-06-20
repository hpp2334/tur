use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::TextElement;
use tur_integration_tests::TurTestApp;

fn build_drag(app: &mut TurTestApp) -> ElementNodeId {
    app.load_bundle("pointer-drag").unwrap();
    app.query_element(&["drag-phase"]).unwrap()
}

fn find_pointer_interact(app: &TurTestApp) -> ElementNodeId {
    let tree = app.element_tree();
    let root = tree.root().unwrap();
    let col = tree.get(root.children[0]).unwrap();
    col.children[0]
}

fn span_content(app: &TurTestApp, id: ElementNodeId) -> String {
    app.with_element(id, |e| {
        e.cast::<TextElement>()
            .map(|tc| {
                tc.spans()
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
            })
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn flush(app: &mut TurTestApp) {
    for _ in 0..6 {
        let _ = app.tick();
    }
}

#[test]
fn drag_emits_down_move_up() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let phase_id = build_drag(&mut app);
    let pos_id = app.query_element(&["drag-pos"]).unwrap();
    let pi_id = find_pointer_interact(&app);

    app.render();

    let bounds = app.get_element_absolute_bounds(pi_id).unwrap();
    let cx = (bounds.left + bounds.right) / 2.0;
    let cy = (bounds.top + bounds.bottom) / 2.0;

    // Down — phase becomes "down", position recorded at (cx, cy).
    app.pointer_down(cx, cy);
    flush(&mut app);
    assert_eq!(span_content(&app, phase_id), "down");
    assert_eq!(span_content(&app, pos_id), format!("{cx:.0},{cy:.0}").replace(".0", ""));

    // Move while dragging — phase becomes "move".
    app.pointer_move(cx + 20.0, cy + 5.0);
    flush(&mut app);
    assert_eq!(span_content(&app, phase_id), "move");

    // Up — phase becomes "up".
    app.pointer_up(cx + 20.0, cy + 5.0);
    flush(&mut app);
    assert_eq!(span_content(&app, phase_id), "up");
}

#[test]
fn hover_move_without_down_does_not_fire_move_event() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let phase_id = build_drag(&mut app);
    let pi_id = find_pointer_interact(&app);

    app.render();

    // Initial state.
    assert_eq!(span_content(&app, phase_id), "idle");

    // Hover move (no button) — onPointerMove must NOT fire (it's drag-only).
    let bounds = app.get_element_absolute_bounds(pi_id).unwrap();
    let cx = (bounds.left + bounds.right) / 2.0;
    let cy = (bounds.top + bounds.bottom) / 2.0;
    app.pointer_move(cx + 10.0, cy + 10.0);
    flush(&mut app);

    assert_eq!(
        span_content(&app, phase_id),
        "idle",
        "onPointerMove should not fire on hover without button down"
    );
}
