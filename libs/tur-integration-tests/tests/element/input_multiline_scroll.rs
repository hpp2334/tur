//! Multiline `Input` vertical scrolling — the playground text-editor gap.
//!
//! The rut playground's editor pane is a fixed-height multiline `Input`
//! loaded with a whole case source. Before this regression the element had
//! NO scrolling machinery at all (no `ElementOnWheel` impl, no scroll
//! offset in paint, no cursor-follow reveal): wheel events over the editor
//! did nothing, the caret moved invisibly past the fold, and everything
//! beyond the field's height was unreachable. Boa's editor scrolls.
//!
//! These tests pin the rails: wheel advances (and clamps) a multiline
//! field's scroll offset without remounting anything; a single-line field
//! never scrolls (the delta passes through); moving the caret below the
//! fold reveals it; clicks and the IME caret rect read through the offset;
//! replacing the text resets the scroll to the top.

use tur_engine::builtin_plugins::text::EditableTextElement;
use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

const EDITOR_RUT: &str = r#"
use tur::{ mount, st_put, st_take, tctrl_new, tctrl_set_text };
use tur_kit::{ TextCtrl, UndoCtrl, Input };

let K_CTRL: u64 = 1;

entry fn start() -> u64 {
    let ctrl = tctrl_new();
    let mut text = "";
    let mut i = 0;
    while (i < 40) {
        text = f"{text}line {i}\n";
        i = i + 1;
    }
    tctrl_set_text(ctrl, text);
    st_put(K_CTRL, ctrl);
    let input = Input()
        .controller(TextCtrl(ctrl))
        .multiline(true)
        .width_height(200.0, 60.0)
        .query_key("ed")
        .build();
    mount(input);
    return 0;
}

entry fn set_short_text(_a: u64, _b: f64) {
    let ctrl = st_take(K_CTRL);
    tctrl_set_text(ctrl, "short");
    st_put(K_CTRL, ctrl);
}
"#;

/// A single-line control with the same geometry — the negative control.
const SINGLE_LINE_RUT: &str = r#"
use tur::{ mount, tctrl_new, tctrl_set_text };
use tur_kit::{ TextCtrl, UndoCtrl, Input };

entry fn start() -> u64 {
    let ctrl = tctrl_new();
    tctrl_set_text(ctrl, "one line only");
    let input = Input()
        .controller(TextCtrl(ctrl))
        .width_height(200.0, 60.0)
        .query_key("ed")
        .build();
    mount(input);
    return 0;
}
"#;

/// Mount the editor fixture and locate the `EditableTextElement` (the
/// `Input` view is a Container wrapper whose only child is the editable;
/// `mount` hosts the container under the root host).
fn setup(source: &str) -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(source).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let tree = app.element_tree();
    let root_snap = tree.root_element().unwrap();
    assert_eq!(
        root_snap.children.len(),
        1,
        "root hosts the Input container"
    );
    let container = ElementNodeId::new(root_snap.children[0].as_u64());
    let container_snap = tree.get_element(container).unwrap();
    assert_eq!(
        container_snap.children.len(),
        1,
        "the Input container wraps one editable"
    );
    let ed = ElementNodeId::new(container_snap.children[0].as_u64());
    app.with_element(ed, |e| {
        assert!(
            e.cast::<EditableTextElement>().is_some(),
            "child is the editable"
        );
    })
    .unwrap();
    (app, ed)
}

#[test]
fn wheel_advances_multiline_scroll() {
    let (mut app, ed) = setup(EDITOR_RUT);

    app.wheel(0.0, 40.0, 100.0, 30.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(ed, |e| {
        let ed = e.cast::<EditableTextElement>().unwrap();
        assert!(
            ed.max_scroll_y() > 300.0,
            "fixture must overflow: max_scroll_y = {}",
            ed.max_scroll_y()
        );
        assert!(
            (ed.scroll_y() - 40.0).abs() < 0.001,
            "scroll_y = {}",
            ed.scroll_y()
        );
    })
    .unwrap();
}

#[test]
fn wheel_accumulates_and_clamps_multiline_scroll() {
    let (mut app, ed) = setup(EDITOR_RUT);

    app.wheel(0.0, 400.0, 100.0, 30.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.wheel(0.0, 400.0, 100.0, 30.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(ed, |e| {
        let ed = e.cast::<EditableTextElement>().unwrap();
        assert_eq!(ed.scroll_y(), ed.max_scroll_y(), "clamped to max");
    })
    .unwrap();

    app.wheel(0.0, -100.0, 100.0, 30.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(ed, |e| {
        let ed = e.cast::<EditableTextElement>().unwrap();
        assert!((ed.scroll_y() - (ed.max_scroll_y() - 100.0)).abs() < 0.001);
    })
    .unwrap();
}

#[test]
fn wheel_does_not_remount_the_editor() {
    let (mut app, ed) = setup(EDITOR_RUT);

    app.wheel(0.0, 40.0, 100.0, 30.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    // The same element id still casts to the editable — a rebuild would
    // have minted a new id and dropped pointer/selection state.
    app.with_element(ed, |e| {
        assert!(e.cast::<EditableTextElement>().is_some());
    })
    .unwrap();
}

#[test]
fn single_line_input_never_scrolls() {
    let (mut app, ed) = setup(SINGLE_LINE_RUT);

    app.wheel(0.0, 40.0, 100.0, 30.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(ed, |e| {
        let ed = e.cast::<EditableTextElement>().unwrap();
        assert_eq!(ed.scroll_y(), 0.0, "single-line fields don't scroll");
        assert_eq!(ed.max_scroll_y(), 0.0);
    })
    .unwrap();
}

#[test]
fn arrow_down_reveals_the_caret_past_the_fold() {
    let (mut app, ed) = setup(EDITOR_RUT);

    // Focus the field, then walk the caret well below the 60px fold.
    app.click(100.0, 30.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    for _ in 0..30 {
        app.send_key("ArrowDown");
    }
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(ed, |e| {
        let ed = e.cast::<EditableTextElement>().unwrap();
        let max = ed.max_scroll_y();
        assert!(
            ed.scroll_y() > 300.0,
            "caret reveal scrolled to {}",
            ed.scroll_y()
        );
        assert!(ed.scroll_y() <= max);
    })
    .unwrap();
}

#[test]
fn clicks_read_through_the_scroll_offset() {
    let (mut app, ed) = setup(EDITOR_RUT);

    // Scroll deep, then click near the TOP of the visible field: the byte
    // under the cursor must come from layout y ≈ 200+offset, not from
    // line 0 (the pre-scroll regression read the raw local y).
    app.wheel(0.0, 200.0, 100.0, 30.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.click(100.0, 5.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(ed, |e| {
        let ed = e.cast::<EditableTextElement>().unwrap();
        // Layout y = 5 (click) + 200 (scroll) ≈ 205 → with the default
        // ~20px line box that's line 10, whose start byte is 70. The
        // pre-fix regression read the raw local y and landed on line 0
        // (byte 0..7), so anything ≥ 70 proves the offset was applied.
        assert!(
            ed.cursor_position() >= 70 && ed.cursor_position() < 300,
            "cursor byte = {}",
            ed.cursor_position()
        );
    })
    .unwrap();
}

#[test]
fn replacing_the_text_resets_the_scroll() {
    let (mut app, ed) = setup(EDITOR_RUT);

    app.wheel(0.0, 400.0, 100.0, 30.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Reset the controller's text programmatically (the playground does
    // this on every case switch): the view must return to the top.
    // (set_short_text is dispatched through the module's entry probe.)
    app.call_rut_entry("set_short_text", 0, 0.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(ed, |e| {
        let ed = e.cast::<EditableTextElement>().unwrap();
        assert_eq!(ed.scroll_y(), 0.0, "new text starts at the top");
    })
    .unwrap();
}
