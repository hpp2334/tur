use tur_engine::builtin_plugins::text::elements::EditableTextElement;
use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

/// Locate the `EditableTextElement` nested under the element tagged with the
/// given `queryKey` (Input puts the queryKey on its Container wrapper; the
/// editable text is that container's first child).
fn find_editable_under(app: &TurTestApp, key: &[&str]) -> ElementNodeId {
    let container_id = app.query_element(key).expect("queryKey not found");
    let container_id = container_id.as_element_id();
    let tree = app.element_tree();
    let container = tree.get_element(container_id).unwrap();
    for cid in container.children.iter().copied() {
        let node = tree.get_element(cid.as_element_id()).unwrap();
        if node.kind() == Some(ElementKind::new("tur_editable_text")) {
            return cid.as_element_id();
        }
    }
    panic!("no tur_editable_text under queryKey {:?}", key);
}

fn get_text(app: &TurTestApp, id: ElementNodeId) -> String {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.text())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn get_selection(app: &TurTestApp, id: ElementNodeId) -> (usize, usize) {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.selection())
            .unwrap_or((0, 0))
    })
    .unwrap_or((0, 0))
}

fn get_cursor(app: &TurTestApp, id: ElementNodeId) -> usize {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.cursor_position())
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

fn focus_editable(app: &mut TurTestApp, id: ElementNodeId) {
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
}

/// Inline bundle that places a single Input at the top-left of the
/// canvas. Reused across tests to avoid the JS bundle roundtrip.
const INPUT_BUNDLE: &str = r#"
    import { createTextEditingController, setViewRoot, viewRoot, Container, Input } from "tur:std";
    const controller = createTextEditingController({});
    setViewRoot(viewRoot("main"), Container({
        children: [
            Input({
                controller: controller,
                fontSize: 14,
                width: 200,
                height: 30,
                queryKey: ["input"],
            }),
        ],
    }));
"#;

fn setup_focused_input_with(text: &str) -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(INPUT_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let input_id = find_editable_under(&app, &["input"]);
    focus_editable(&mut app, input_id);
    for ch in text.chars() {
        app.send_key(&ch.to_string());
        app.wait_for_timeout(std::time::Duration::ZERO);
    }
    app.wait_for_timeout(std::time::Duration::ZERO);
    (app, input_id)
}

/// Select a range of characters using Shift+ArrowLeft / ArrowRight, starting
/// from `start_byte` (assumed to be set via Home / End navigation).
fn select_range(app: &mut TurTestApp, _id: ElementNodeId, from_byte: usize, to_byte: usize) {
    // Move cursor to from_byte (assume Home then ArrowRight * from_byte).
    app.send_key("Home");
    app.wait_for_timeout(std::time::Duration::ZERO);
    for _ in 0..from_byte {
        app.send_key("ArrowRight");
        app.wait_for_timeout(std::time::Duration::ZERO);
    }
    // Shift+ArrowRight to extend to to_byte.
    let delta = to_byte as i64 - from_byte as i64;
    if delta >= 0 {
        for _ in 0..delta {
            app.send_key_with_modifiers("ArrowRight", true, false);
            app.wait_for_timeout(std::time::Duration::ZERO);
        }
    } else {
        for _ in 0..(-delta) {
            app.send_key_with_modifiers("ArrowLeft", true, false);
            app.wait_for_timeout(std::time::Duration::ZERO);
        }
    }
    app.wait_for_timeout(std::time::Duration::ZERO);
}

// ---------------------------------------------------------------------------
// Copy (Cmd+C)
// ---------------------------------------------------------------------------

#[test]
fn cmd_c_copies_selected_text_to_clipboard_slot() {
    let (mut app, id) = setup_focused_input_with("hello");
    assert_eq!(get_text(&app, id), "hello");

    // Select "ll" (bytes 2..4).
    select_range(&mut app, id, 2, 4);
    let (anchor, end) = get_selection(&app, id);
    assert_eq!((anchor, end), (2, 4));

    // No pending write before Cmd+C.
    assert!(app.take_clipboard_write().is_none());

    // Cmd+C copies the selected text.
    app.send_key_with_modifiers_full("c", false, false, true);
    app.wait_for_timeout(std::time::Duration::ZERO);

    let written = app.take_clipboard_write();
    assert_eq!(
        written.as_deref(),
        Some("ll"),
        "Cmd+C should write the selected text to the clipboard slot"
    );

    // Subsequent polls return None (slot is drained).
    assert!(app.take_clipboard_write().is_none());

    // Selection is preserved after Cmd+C (copy does not modify text).
    let (anchor2, end2) = get_selection(&app, id);
    assert_eq!((anchor2, end2), (2, 4));
    assert_eq!(get_text(&app, id), "hello");
}

#[test]
fn cmd_c_with_no_selection_writes_nothing() {
    let (mut app, id) = setup_focused_input_with("abc");
    // Move cursor to start (no selection).
    app.send_key("Home");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert!(!{
        let (a, b) = get_selection(&app, id);
        a != b
    });

    app.send_key_with_modifiers_full("c", false, false, true);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(
        app.take_clipboard_write().is_none(),
        "Cmd+C with no selection should not write to the clipboard"
    );
}

// ---------------------------------------------------------------------------
// Cut (Cmd+X)
// ---------------------------------------------------------------------------

#[test]
fn cmd_x_cuts_selected_text_to_clipboard_slot() {
    let (mut app, id) = setup_focused_input_with("hello");
    select_range(&mut app, id, 2, 4);

    app.send_key_with_modifiers_full("x", false, false, true);
    app.wait_for_timeout(std::time::Duration::ZERO);

    let written = app.take_clipboard_write();
    assert_eq!(
        written.as_deref(),
        Some("ll"),
        "Cmd+X should write the cut text to the clipboard slot"
    );

    assert_eq!(
        get_text(&app, id),
        "heo",
        "Cmd+X should delete the selection from the buffer"
    );
    assert_eq!(
        get_cursor(&app, id),
        2,
        "Cursor should land at the cut start position"
    );
}

// ---------------------------------------------------------------------------
// Paste (Cmd+V → ClipboardPasteEvent via AppEvent::Custom)
// ---------------------------------------------------------------------------

#[test]
fn paste_event_inserts_at_cursor() {
    let (mut app, id) = setup_focused_input_with("abc");
    // Move cursor between 'b' and 'c' (position 2).
    app.send_key("Home");
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.send_key("ArrowRight");
    app.send_key("ArrowRight");
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Simulate the embedder firing a paste event.
    app.push_paste_event("XY");
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        get_text(&app, id),
        "abXYc",
        "Paste should insert at the cursor"
    );
    assert_eq!(
        get_cursor(&app, id),
        4,
        "Cursor should advance past the inserted text"
    );
}

#[test]
fn paste_event_replaces_selection() {
    let (mut app, id) = setup_focused_input_with("hello world");
    // Select "world" (bytes 6..11).
    select_range(&mut app, id, 6, 11);

    app.push_paste_event("there");
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        get_text(&app, id),
        "hello there",
        "Paste over a selection should replace it"
    );
    assert_eq!(
        get_cursor(&app, id),
        11,
        "Cursor should land at the end of the pasted text"
    );
}

// ---------------------------------------------------------------------------
// Round-trip: Cmd+X then paste restores the original text
// ---------------------------------------------------------------------------

#[test]
fn cut_then_paste_roundtrips_text() {
    let (mut app, id) = setup_focused_input_with("hello");
    select_range(&mut app, id, 0, 5);

    // Cut the whole word.
    app.send_key_with_modifiers_full("x", false, false, true);
    app.wait_for_timeout(std::time::Duration::ZERO);
    let cut_text = app.take_clipboard_write().expect("Cmd+X should write");
    assert_eq!(cut_text, "hello");
    assert_eq!(get_text(&app, id), "");

    // Re-paste via the embedder paste path. In the test harness we don't
    // have a real system clipboard, so we push the cut text back through
    // the paste event channel.
    app.push_paste_event(&cut_text);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        get_text(&app, id),
        "hello",
        "Pasting the cut text back should restore the buffer"
    );
}
