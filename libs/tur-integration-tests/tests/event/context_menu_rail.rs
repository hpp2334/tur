use tur_engine::builtin_plugins::text::elements::EditableTextElement;
use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

// ---------------------------------------------------------------------------
// The editable's context-menu rail (the kit surface): `Input.on_context_menu`
// installs the sealed mutation; the engine's right-button gesture fires it
// with the typed `ContextMenuEvent` (local + global points). This is the
// playground menu's open trigger — the boa-era `onContextMenu: openContextMenu`
// wiring, re-activated.
// ---------------------------------------------------------------------------

const CTX_MENU_RUT: &str = r#"
use tur_kit::gesture::pointer::{ ContextMenuEvent };
use tur_kit::handles::{ mount, text_ctrl, undo_ctrl }; use tur_kit::layout::flex::{ Column }; use tur_kit::reactive::{ MutationCtx, Source, entry_ctx, mutate, source }; use tur_kit::text::core::{ Text }; use tur_kit::text::input::{ Input };

struct AppContext {
    label: Source<str>,
}

fn start() -> AppContext {
    let label = source<str>("");
    let input = Input().controller(text_ctrl()).undo(undo_ctrl()).width_height(300.0, 30.0)
        .on_context_menu(mutate<ContextMenuEvent>(fn (ctx: MutationCtx, ev: ContextMenuEvent) {
            ctx.set<str>(label, f"menu g{ev.global.x as u64},{ev.global.y as u64} l{ev.local.x as u64},{ev.local.y as u64}");
        }))
        .query_key("input").build();
    let col = Column().child(input).child(Text().text(label).query_key("rut/text").build());
    mount(col.build());
    return AppContext { label: label };
}

entry fn entry_start() -> opaque {
    return opaque(start());
}
"#;

fn find_editable(app: &TurTestApp) -> ElementNodeId {
    let container_id = app.query_element(&["input"]).expect("queryKey not found");
    let container_id = ElementNodeId::new(container_id.as_u64());
    let tree = app.element_tree();
    let container = tree.get_element(container_id).unwrap();
    for cid in container.children.iter().copied() {
        let node = tree.get_element(ElementNodeId::new(cid.as_u64())).unwrap();
        if node.kind() == Some(ElementKind::new("tur_editable_text")) {
            return ElementNodeId::new(cid.as_u64());
        }
    }
    panic!("no tur_editable_text under queryKey input");
}

fn label_of(app: &TurTestApp) -> String {
    let id = app
        .query_element(&["rut", "text"])
        .expect("bound text not found");
    let id = ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<tur_engine::builtin_plugins::text::TextElement>()
            .map(|c| {
                c.spans()
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
            })
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn selection_of(app: &TurTestApp, id: ElementNodeId) -> (usize, usize) {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.selection())
            .unwrap_or((0, 0))
    })
    .unwrap_or((0, 0))
}

/// Right-click on the editor fires the on_context_menu mutation with the
/// typed event — the global point is canvas-relative (the menu anchor).
#[test]
fn input_on_context_menu_fires_with_points() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(CTX_MENU_RUT).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let _cx = app.call_rut_entry_opaque("entry_start").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let input_id = find_editable(&app);
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let (cx, cy) = bounds.center();

    // Focus and type so the editor is live.
    app.click(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
    for ch in "hello world".chars() {
        app.send_key(&ch.to_string());
        app.wait_for_timeout(std::time::Duration::ZERO);
    }
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Right-click inside the editor: the mutation fires with the typed
    // points (the label carries the truncated ints — `as u64` drops the
    // fraction, so the expected string truncates the same way).
    app.right_click(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
    let transcript = label_of(&app);
    let want = format!(
        "menu g{},{} l{},{}",
        cx as u64,
        cy as u64,
        (cx - bounds.left) as u64,
        (cy - bounds.top) as u64
    );
    assert_eq!(
        transcript, want,
        "the context-menu mutation fired with the typed points"
    );
}

/// Right-click INSIDE an active selection preserves it (the menu's Cut/Copy
/// must operate on the selection the user right-clicked) — the native-OS
/// semantics the gesture pairs with the rail.
#[test]
fn right_click_inside_selection_still_preserves_with_menu_rail() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(CTX_MENU_RUT).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let _cx = app.call_rut_entry_opaque("entry_start").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let input_id = find_editable(&app);
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let (cx, cy) = bounds.center();

    app.click(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
    for ch in "hello world".chars() {
        app.send_key(&ch.to_string());
        app.wait_for_timeout(std::time::Duration::ZERO);
    }
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Select "world" (bytes 6..11): Home + 6×ArrowRight + 5×Shift+ArrowRight.
    app.send_key("Home");
    app.wait_for_timeout(std::time::Duration::ZERO);
    for _ in 0..6 {
        app.send_key("ArrowRight");
        app.wait_for_timeout(std::time::Duration::ZERO);
    }
    for _ in 0..5 {
        app.send_key_with_modifiers("ArrowRight", true, false);
        app.wait_for_timeout(std::time::Duration::ZERO);
    }
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(
        selection_of(&app, input_id),
        (6, 11),
        "precondition: 'world' selected"
    );

    // Right-click inside the selection (~75% across): the selection must
    // survive AND the menu rail must fire.
    let inside_x = bounds.left + (bounds.right - bounds.left) * 0.75;
    app.right_click(inside_x, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        selection_of(&app, input_id),
        (6, 11),
        "right-click inside the selection preserves it"
    );
    assert!(
        label_of(&app).starts_with("menu g"),
        "the context-menu mutation fired alongside the preserved selection"
    );
}
