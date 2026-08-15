use tur_engine::builtin_plugins::text::elements::EditableTextElement;
use tur_engine::core::element::ElementKind;
use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// Inline bundle that mounts a single `Input` with `obscureText: true`. The
/// `queryKey` lands on Input's Container wrapper; the editable text is that
/// container's first child.
const PASSWORD_BUNDLE: &str = r#"
    import { createTextEditingController, setViewRoot, viewRoot, Container, Input } from "tur:std";
    const controller = createTextEditingController({});
    setViewRoot(viewRoot("main"), Container({
        children: [
            Input({
                controller: controller,
                fontSize: 14,
                width: 200,
                height: 30,
                obscureText: true,
                queryKey: ["input"],
            }),
        ],
    }));
"#;

const CUSTOM_CHAR_BUNDLE: &str = r#"
    import { createTextEditingController, setViewRoot, viewRoot, Container, Input } from "tur:std";
    const controller = createTextEditingController({});
    setViewRoot(viewRoot("main"), Container({
        children: [
            Input({
                controller: controller,
                fontSize: 14,
                width: 200,
                height: 30,
                obscureText: true,
                obscuringCharacter: "*",
                queryKey: ["input"],
            }),
        ],
    }));
"#;

fn find_editable(app: &TurTestApp, key: &[&str]) -> ElementNodeId {
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

fn focus(app: &mut TurTestApp, id: ElementNodeId) {
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
}

fn get_value(app: &TurTestApp, id: ElementNodeId) -> String {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.text())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn get_displayed(app: &TurTestApp, id: ElementNodeId) -> String {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.displayed_text())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn get_cursor(app: &TurTestApp, id: ElementNodeId) -> usize {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.cursor_position())
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

fn cursor_x(app: &TurTestApp, id: ElementNodeId, byte: usize) -> f32 {
    app.with_element(id, move |e| {
        e.cast::<EditableTextElement>()
            .and_then(|el| el.cursor_x_at(byte))
            .unwrap_or(0.0)
    })
    .unwrap_or(0.0)
}

fn type_str(app: &mut TurTestApp, s: &str) {
    for ch in s.chars() {
        app.send_key(&ch.to_string());
        app.wait_for_timeout(std::time::Duration::ZERO);
    }
}

// ---------------------------------------------------------------------------
// Masking: typed text is masked on display but the controller keeps the value
// ---------------------------------------------------------------------------

#[test]
fn password_masks_typed_text_but_keeps_value() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(PASSWORD_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    focus(&mut app, id);
    app.wait_for_timeout(std::time::Duration::ZERO);

    type_str(&mut app, "abc");
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        get_value(&app, id),
        "abc",
        "controller keeps the real value"
    );
    assert_eq!(get_displayed(&app, id), "•••", "display is masked");
    assert_eq!(
        get_cursor(&app, id),
        3,
        "cursor advances in value-byte space"
    );
}

#[test]
fn password_backspace_removes_a_mask_char() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(PASSWORD_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    focus(&mut app, id);
    type_str(&mut app, "abc");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(get_displayed(&app, id), "•••");

    app.send_key("Backspace");
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(get_value(&app, id), "ab");
    assert_eq!(get_displayed(&app, id), "••");
}

#[test]
fn password_empty_value_displays_nothing() {
    let app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(PASSWORD_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    assert_eq!(get_value(&app, id), "");
    assert_eq!(get_displayed(&app, id), "");
}

// ---------------------------------------------------------------------------
// Clipboard exfiltration guard: copy/cut are suppressed when obscured
// ---------------------------------------------------------------------------

#[test]
fn password_copy_is_suppressed() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(PASSWORD_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    focus(&mut app, id);
    type_str(&mut app, "hello");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(get_value(&app, id), "hello");

    // Select all.
    app.send_key_with_modifiers_full("a", false, false, true);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(
        app.take_clipboard_write().is_none(),
        "no pending write before Cmd+C"
    );

    app.send_key_with_modifiers_full("c", false, false, true);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(
        app.take_clipboard_write().is_none(),
        "Cmd+C must not write the password to the clipboard"
    );
    assert_eq!(get_value(&app, id), "hello", "value unchanged after Cmd+C");
}

#[test]
fn password_cut_is_suppressed() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(PASSWORD_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    focus(&mut app, id);
    type_str(&mut app, "hello");
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.send_key_with_modifiers_full("a", false, false, true);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.send_key_with_modifiers_full("x", false, false, true);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(
        app.take_clipboard_write().is_none(),
        "Cmd+X must not write the password to the clipboard"
    );
    assert_eq!(
        get_value(&app, id),
        "hello",
        "Cmd+X must not delete the buffer in password mode"
    );
}

// ---------------------------------------------------------------------------
// Configurable obscuring character
// ---------------------------------------------------------------------------

#[test]
fn password_custom_obscuring_character() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(CUSTOM_CHAR_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    focus(&mut app, id);
    type_str(&mut app, "abc");
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(get_value(&app, id), "abc");
    assert_eq!(get_displayed(&app, id), "***");
}

// ---------------------------------------------------------------------------
// Non-ASCII values: one mask char per character, grapheme nav stays valid
// ---------------------------------------------------------------------------

#[test]
fn password_multibyte_value_masks_one_bullet_per_char() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(PASSWORD_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    focus(&mut app, id);
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Multi-byte text arrives via paste (single-byte keystrokes are the only
    // printable keys the keyboard path inserts; multi-byte chars come through
    // IME / paste). '猫' is 3 UTF-8 bytes; one character → one bullet.
    app.push_paste_event("猫");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(get_value(&app, id), "猫");
    assert_eq!(get_displayed(&app, id), "•");
    assert_eq!(get_cursor(&app, id), 3, "cursor at byte 3 (end of one 猫)");

    app.send_key("Home");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(get_cursor(&app, id), 0);
    app.send_key("ArrowRight");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(
        get_cursor(&app, id),
        3,
        "ArrowRight advances one grapheme (3 bytes)"
    );

    // A mixed multi-byte value still masks one char per glyph.
    app.send_key("Home");
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.push_paste_event("a猫b");
    app.wait_for_timeout(std::time::Duration::ZERO);
    // value was "猫"; pasting "a猫b" at byte 0 → "a猫b猫" (4 chars).
    assert_eq!(get_value(&app, id), "a猫b猫");
    assert_eq!(get_displayed(&app, id), "••••", "4 chars → 4 bullets");
}

// ---------------------------------------------------------------------------
// Offset translation: a click resolves to a value-byte offset even though the
// masked display string has a different byte length than the value.
// ---------------------------------------------------------------------------

#[test]
fn password_click_resolves_in_value_byte_space() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(PASSWORD_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    focus(&mut app, id);
    // 6 ASCII chars (6 value bytes) → 6 bullets (18 display bytes). A click
    // must land in the [0, 6] value-byte range, not the [0, 18] display-byte
    // range.
    type_str(&mut app, "abcdef");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(get_displayed(&app, id), "••••••");

    // x of the caret between 'c' and 'd' (value byte 3) in the remapped layout.
    let x3 = cursor_x(&app, id, 3);
    let bounds = app.get_element_absolute_bounds(id).unwrap();
    let cy = (bounds.top + bounds.bottom) * 0.5;

    app.send_key("Home");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(get_cursor(&app, id), 0);

    // Click just past the byte-3 boundary.
    app.pointer_down(bounds.left + x3 as f64 + 0.5, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
    let clicked = get_cursor(&app, id);
    assert!(
        clicked <= 6,
        "click cursor must stay in value-byte range [0,6], got {clicked}"
    );

    // Inserting at the clicked position must yield a valid char-boundary split
    // in value space — proves the remap gave a value byte, not a display byte.
    app.send_key("Z");
    app.wait_for_timeout(std::time::Duration::ZERO);
    let text = get_value(&app, id);
    assert!(text.starts_with("abc"), "prefix preserved: {text}");
    assert!(
        text.len() == 7 && text.contains('Z'),
        "inserted one char in value space: {text}"
    );
    assert_eq!(text, "abcZdef", "inserted at value byte 3");
}

// ---------------------------------------------------------------------------
// Multi-code-point graphemes: one bullet per *grapheme*, not per code point.
// Combining marks, flag emoji, and ZWJ sequences are each a single grapheme
// cluster (UAX #29) and must collapse to a single bullet — matching the
// engine's own grapheme-based cursor/backspace model and Flutter/web password
// fields.
// ---------------------------------------------------------------------------

#[test]
fn password_combining_mark_is_one_bullet() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(PASSWORD_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    focus(&mut app, id);
    app.wait_for_timeout(std::time::Duration::ZERO);

    // 'é' as 'e' + U+0301 combining acute: 2 code points, 1 grapheme.
    app.push_paste_event("e\u{0301}");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(get_value(&app, id), "e\u{0301}");
    assert_eq!(get_displayed(&app, id), "•", "one grapheme → one bullet");

    // Backspace deletes the whole grapheme: both code points go, display empty.
    app.send_key("Backspace");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(get_value(&app, id), "", "whole grapheme deleted");
    assert_eq!(get_displayed(&app, id), "");
}

#[test]
fn password_flag_emoji_is_one_bullet() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(PASSWORD_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    focus(&mut app, id);
    app.wait_for_timeout(std::time::Duration::ZERO);

    // US flag = U+1F1FA U+1F1F8 (regional indicators): 2 code points, 1 grapheme.
    app.push_paste_event("\u{1F1FA}\u{1F1F8}");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(get_value(&app, id), "\u{1F1FA}\u{1F1F8}");
    assert_eq!(get_displayed(&app, id), "•", "flag emoji → one bullet");
}

#[test]
fn password_mixed_graphemes_mask_per_grapheme() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(PASSWORD_BUNDLE).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let id = find_editable(&app, &["input"]);
    focus(&mut app, id);
    app.wait_for_timeout(std::time::Duration::ZERO);

    // 'a' + US flag + 'b' = 3 graphemes → 3 bullets, even though it's
    // 1 + 2 + 1 = 4 code points.
    app.push_paste_event("a\u{1F1FA}\u{1F1F8}b");
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(get_value(&app, id), "a\u{1F1FA}\u{1F1F8}b");
    assert_eq!(get_displayed(&app, id), "•••", "3 graphemes → 3 bullets");
}
