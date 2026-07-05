use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_std::elements::EditableTextElement;
use tur_integration_tests::TurTestApp;

/// Locate the `EditableTextElement` nested under the element tagged with the
/// given `queryKey` (InputEdgy puts the queryKey on its Container wrapper; the
/// editable text is that container's first child).
fn find_editable_under(app: &TurTestApp, key: &[&str]) -> ElementNodeId {
    let container_id = app.query_element(key).expect("queryKey not found");
    let container_id = ElementNodeId::new(container_id.as_u64());
    let tree = app.element_tree();
    let container = tree.get_element(container_id).unwrap();
    for cid in container.children.iter().copied() {
        let node = tree.get_element(ElementNodeId::new(cid.as_u64())).unwrap();
        if node
            .element
            .as_ref()
            .map(|e| e.kind() == ElementKind::new("tur_editable_text"))
            .unwrap_or(false)
        {
            return ElementNodeId::new(cid.as_u64());
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

fn focus_editable(app: &mut TurTestApp, id: ElementNodeId) {
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
}

/// Inline bundle that places a single InputEdgy at the top-left of the canvas,
/// wired up with an `UndoController` (mirrors the playground editor config).
const UNDO_INPUT_BUNDLE: &str = r#"
    import { createTextEditingController, createUndoController, render, Container, InputEdgy } from "builtin:tur/std";
    globalThis.__ctrl = createTextEditingController({});
    globalThis.__undo = createUndoController();
    render(Container({
        children: [
            InputEdgy({
                controller: globalThis.__ctrl,
                undoController: globalThis.__undo,
                multiline: true,
                fontFamily: "monospace",
                fontSize: 14,
                width: 400,
                height: 200,
                queryKey: ["input"],
            }),
        ],
    }));
"#;

/// Bundle that mirrors the playground editor: every `onInput` re-tokenizes via
/// `setSpansPreserveCursor`. Used to reproduce the demo's "select all → cut →
/// undo does nothing" bug at the engine level.
const PLAYGROUND_BUNDLE: &str = r#"
    import { mutate, createTextEditingController, createUndoController, render, Container, InputEdgy } from "builtin:tur/std";
    // Tokenize the buffer into a single plain span (no syntax highlighting —
    // the act of calling setSpansPreserveCursor on every input is what matters).
    const onInput = mutate((_ctxArg) => {
        globalThis.__ctrl.setSpansPreserveCursor(
            [{ content: globalThis.__ctrl.text }],
        );
    });
    globalThis.__ctrl = createTextEditingController({
        onInput: onInput,
    });
    globalThis.__undo = createUndoController();
    render(Container({
        children: [
            InputEdgy({
                controller: globalThis.__ctrl,
                undoController: globalThis.__undo,
                multiline: true,
                fontFamily: "monospace",
                fontSize: 14,
                width: 400,
                height: 200,
                queryKey: ["input"],
            }),
        ],
    }));
"#;

fn setup() -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(500.0, 400.0).unwrap();
    app.eval_module_source(UNDO_INPUT_BUNDLE).unwrap();
    app.render();
    let id = find_editable_under(&app, &["input"]);
    focus_editable(&mut app, id);
    app.render();
    (app, id)
}

fn setup_playground() -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(500.0, 400.0).unwrap();
    app.eval_module_source(PLAYGROUND_BUNDLE).unwrap();
    app.render();
    let id = find_editable_under(&app, &["input"]);
    focus_editable(&mut app, id);
    app.render();
    (app, id)
}

// ---------------------------------------------------------------------------
// Regression: Cmd+A → Cmd+X → Cmd+Z must restore the cut text.
// Reported playground bug: after selecting all and cutting, pressing Cmd+Z
// does NOT bring the text back.
// ---------------------------------------------------------------------------

#[test]
fn select_all_cut_then_undo_restores_text() {
    let (mut app, id) = setup();

    // Type some text.
    for ch in "hello world".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();
    assert_eq!(get_text(&app, id), "hello world");

    // Cmd+A — select everything.
    app.send_key_with_modifiers_full("a", false, false, true);
    app.render();
    let (anchor, end) = get_selection(&app, id);
    let (lo, hi) = if anchor <= end { (anchor, end) } else { (end, anchor) };
    assert_eq!(lo, 0);
    assert_eq!(hi, "hello world".len(), "Cmd+A should select the whole buffer");

    // Cmd+X — cut.
    app.send_key_with_modifiers_full("x", false, false, true);
    app.render();
    let written = app.take_clipboard_write();
    assert_eq!(written.as_deref(), Some("hello world"),
        "Cmd+X should write the cut text to the clipboard slot");
    assert_eq!(get_text(&app, id), "", "Cmd+X should clear the buffer");

    // Cmd+Z — undo the cut. The buffer MUST be restored.
    app.send_key_with_modifiers_full("z", false, false, true);
    app.render();

    assert_eq!(
        get_text(&app, id),
        "hello world",
        "Cmd+Z after Cmd+A → Cmd+X must restore the cut text",
    );
}

/// Sanity-check the inverse path: typing, then Cmd+Z should undo the last
/// typed char. This rules out a totally broken undo controller and isolates
/// the select-all-and-cut case as the regression.
#[test]
fn undo_after_typing_deletes_last_char() {
    let (mut app, id) = setup();

    for ch in "abc".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();
    assert_eq!(get_text(&app, id), "abc");

    app.send_key_with_modifiers_full("z", false, false, true);
    app.render();
    assert_eq!(get_text(&app, id), "ab", "Cmd+Z should undo the last typed char");
}

/// Same regression but with a partial selection (not select-all) to confirm
/// that undo-after-cut works in general and the failure is specific to the
/// select-all case.
#[test]
fn partial_cut_then_undo_restores_text() {
    let (mut app, id) = setup();

    for ch in "hello world".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();
    assert_eq!(get_text(&app, id), "hello world");

    // Select "world" (bytes 6..11) via Home + 6×Right, then Shift+5×Right.
    app.send_key("Home");
    app.render();
    for _ in 0..6 {
        app.send_key("ArrowRight");
    }
    for _ in 0..5 {
        app.send_key_with_modifiers("ArrowRight", true, false);
    }
    app.render();
    let (anchor, end) = get_selection(&app, id);
    let (lo, hi) = if anchor <= end { (anchor, end) } else { (end, anchor) };
    assert_eq!((lo, hi), (6, 11), "should have selected 'world'");

    app.send_key_with_modifiers_full("x", false, false, true);
    app.render();
    assert_eq!(get_text(&app, id), "hello ");

    app.send_key_with_modifiers_full("z", false, false, true);
    app.render();
    assert_eq!(
        get_text(&app, id),
        "hello world",
        "Cmd+Z after a partial cut must restore the cut text",
    );
}

// ---------------------------------------------------------------------------
// Faithful playground reproduction: the demo editor's `onInput` callback calls
// `setSpansPreserveCursor` after every buffer mutation. This is the
// configuration under which "select all → cut → undo does nothing" was
// reported.
// ---------------------------------------------------------------------------

#[test]
fn playground_select_all_cut_then_undo_restores_text() {
    let (mut app, id) = setup_playground();

    for ch in "hello world".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();
    assert_eq!(get_text(&app, id), "hello world");

    app.send_key_with_modifiers_full("a", false, false, true);
    app.render();
    app.send_key_with_modifiers_full("x", false, false, true);
    app.render();
    assert_eq!(get_text(&app, id), "");

    app.send_key_with_modifiers_full("z", false, false, true);
    app.render();
    assert_eq!(
        get_text(&app, id),
        "hello world",
        "playground config: Cmd+Z after Cmd+A → Cmd+X must restore the cut text",
    );
}

// ---------------------------------------------------------------------------
// Context-menu path: the playground's Cut menu action calls
// `editorCtrl.deleteSelection()` directly through the JS bridge (see
// tur-demo-impl/src/state/context-menu.ts). Unlike the Cmd+X keyboard path,
// the JS-bridge `deleteSelection` does NOT push a snapshot onto the undo
// stack, so a subsequent Cmd+Z has nothing to restore. This is the
// reproduction for the reported "select all → cut → undo does nothing" bug.
// ---------------------------------------------------------------------------

#[test]
fn context_menu_cut_then_undo_restores_text() {
    let (mut app, id) = setup();

    for ch in "hello world".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();
    assert_eq!(get_text(&app, id), "hello world");

    // Select-all via the controller JS bridge (mirrors the menu's Select All).
    app.eval_js("globalThis.__ctrl.setSelection(0, globalThis.__ctrl.text.length)");
    app.render();
    let (anchor, end) = get_selection(&app, id);
    let (lo, hi) = if anchor <= end { (anchor, end) } else { (end, anchor) };
    assert_eq!((lo, hi), (0, "hello world".len()));

    // Cut via the controller JS bridge (mirrors the menu's Cut action).
    app.eval_js("globalThis.__ctrl.deleteSelection()");
    app.render();
    assert_eq!(get_text(&app, id), "", "deleteSelection should clear the buffer");

    // Undo — should restore, but currently does NOT because the JS-bridge
    // mutation path bypasses the undo stack entirely.
    app.send_key_with_modifiers_full("z", false, false, true);
    app.render();
    assert_eq!(
        get_text(&app, id),
        "hello world",
        "Cmd+Z after a JS-bridge cut must restore the cut text \
         (context-menu Cut should be undoable just like Cmd+X)",
    );
}

// ---------------------------------------------------------------------------
// Context-menu Paste: the playground's Paste menu action calls
// `editorCtrl.insertText(text)` through the JS bridge (see
// tur-demo-impl/src/state/context-menu.ts `pasteFromClipboard`). This must
// also be undoable now that JS-bridge mutations record to the undo stack.
// ---------------------------------------------------------------------------

#[test]
fn context_menu_paste_then_undo_restores_text() {
    let (mut app, id) = setup();

    // Start with "ab".
    app.send_key("a");
    app.send_key("b");
    app.render();
    assert_eq!(get_text(&app, id), "ab");

    // Move cursor between 'a' and 'b' (position 1).
    app.send_key("Home");
    app.send_key("ArrowRight");
    app.render();

    // Paste via the controller JS bridge (mirrors the menu's Paste action).
    app.eval_js("globalThis.__ctrl.insertText('XY')");
    app.render();
    assert_eq!(get_text(&app, id), "aXYb", "paste should insert at cursor");

    // Undo — should remove the pasted text.
    app.send_key_with_modifiers_full("z", false, false, true);
    app.render();
    assert_eq!(
        get_text(&app, id),
        "ab",
        "Cmd+Z after a JS-bridge paste must remove the pasted text",
    );

    // Redo — should re-insert.
    app.send_key_with_modifiers_full("z", true, false, true);
    app.render();
    assert_eq!(
        get_text(&app, id),
        "aXYb",
        "Cmd+Shift+Z must re-apply the undone paste",
    );
}

// ---------------------------------------------------------------------------
// Programmatic `setSpans` with DIFFERENT text must be undoable; with the
// SAME text (re-tokenization for syntax highlighting) must NOT create an
// undo entry. This guards the central use case of the playground editor
// (live re-highlight on every keystroke) and ensures undo entries are only
// created for actual text changes.
// ---------------------------------------------------------------------------

#[test]
fn programmatic_set_spans_with_new_text_is_undoable() {
    let (mut app, id) = setup();

    for ch in "hello".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();
    assert_eq!(get_text(&app, id), "hello");

    // Programmatically replace the buffer with different text.
    app.eval_js(r#"globalThis.__ctrl.setSpans([{ content: "WORLD" }]);"#);
    app.render();
    assert_eq!(get_text(&app, id), "WORLD");

    // Undo — should restore "hello".
    app.send_key_with_modifiers_full("z", false, false, true);
    app.render();
    assert_eq!(
        get_text(&app, id),
        "hello",
        "Cmd+Z after a programmatic setSpans-with-new-text must restore",
    );
}

#[test]
fn set_spans_preserve_cursor_with_same_text_does_not_push_undo() {
    let (mut app, id) = setup();

    // Type "hello" → 5 undo entries (one per keystroke).
    for ch in "hello".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();
    assert_eq!(get_text(&app, id), "hello");

    // Re-tokenize with different span colors but the SAME text — this is
    // exactly what the playground's onInput callback does on every keystroke.
    // It must NOT add undo entries.
    app.eval_js(
        r#"globalThis.__ctrl.setSpansPreserveCursor([
            { content: "he", color: { r: 200, g: 120, b: 50, a: 255 } },
            { content: "llo", color: { r: 80, g: 200, b: 120, a: 255 } },
        ]);"#,
    );
    app.render();
    assert_eq!(get_text(&app, id), "hello");

    // Undo 5 times — should clear the buffer (the 5 typing entries). A 6th
    // undo must do nothing (no extra entry from the re-tokenize).
    for _ in 0..5 {
        app.send_key_with_modifiers_full("z", false, false, true);
        app.render();
    }
    assert_eq!(get_text(&app, id), "", "5 undos should clear all 5 typed chars");

    app.send_key_with_modifiers_full("z", false, false, true);
    app.render();
    assert_eq!(
        get_text(&app, id),
        "",
        "re-tokenize with same text must not have created a 6th undo entry",
    );
}

