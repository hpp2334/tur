//! Phase-A gate for the boa→rut migration: **a rut-only instance never
//! allocates a boa realm.** The realm is constructed lazily — only when a
//! JS module/script actually loads — so a fresh instance that loads HELLO_RUT,
//! clicks a rut button, scrolls a rut view, and reloads stays realm-free
//! (`TurApp::realm_allocated() == false`) for its whole life, while every
//! Rust path (layout, paint, reactive flush, Rust-closure mutations,
//! subsystems) runs unchanged.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

const HELLO_RUT: &str = r#"
use tur::{ el_column, el_text, el_build, el_child, mount };

entry fn start() {
    let col = el_column();
    el_child(col, el_text("hello from rut"));
    el_child(col, el_text("no realm needed"));
    mount(el_build(col));
}
"#;

/// Click: `el_button`'s Rust-closure mutation queues an intent; the pump
/// drains it into `entry fn ts_click`, which writes the bound atom — the
/// interactive loop, realm-free.
const BUTTON_RUT: &str = r#"
use tur::{ el_button, el_column, el_text_bound, el_build, el_child, mount, rs_source_str, rs_set_str };

entry fn start() -> u64 {
    let atom = rs_source_str("taps: 0");
    let col = el_column();
    el_child(col, el_text_bound(atom));
    el_child(col, el_button(atom, atom, "ts_click", "tap me"));
    mount(el_build(col));
    return atom;
}

entry fn ts_click(atom: u64, _label: u64, n: f64) {
    rs_set_str(atom, f"taps: {n}");
}
"#;

/// Scroll: a rut ScrollView with styled rows; wheel events drive the
/// engine's scroll subsystem (Rust path) — no realm involved.
const SCROLL_RUT: &str = r#"
use tur::{ el_column, el_expand, el_scroll, el_text_styled, el_build, el_child, mount };

entry fn start() {
    let content = el_column();
    let mut i = 0;
    while (i < 60) {
        el_child(content, el_text_styled(f"row {i}", 16.0, 0x222222FF));
        i += 1;
    }
    let scroller = el_scroll(true, el_build(content));
    let root = el_column();
    el_child(root, el_expand(1.0, scroller));
    mount(el_build(root));
}
"#;

fn bound_text(app: &TurTestApp) -> String {
    let id = app
        .query_element(&["rut", "text"])
        .expect("bound text not found");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
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

/// The core gate: HELLO_RUT on a fresh instance builds + paints a tree with
/// zero realm allocations.
#[test]
fn rut_only_instance_never_allocates_a_realm() {
    let app = TurTestApp::new(400.0, 600.0).unwrap();
    assert!(
        !app.realm_allocated(),
        "a fresh instance must not pre-allocate a JS realm"
    );

    app.load_rut_module(HELLO_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        !app.realm_allocated(),
        "load_rut_module must not construct the JS realm"
    );

    let root = app.dev_tool_element_tree().expect("rut tree mounted");
    assert_eq!(root.children.len(), 1, "root column is the only child");
    assert!(!app.realm_allocated(), "layout + paint stay realm-free");
}

/// Click journey on a realm-free instance: the rut button's Rust-closure
/// mutation runs through the realm-free mutation path, the intent drains
/// into `ts_click`, and the bound text re-renders — realm stays `None`.
#[test]
fn rut_click_stays_realm_free() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(BUTTON_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        !app.realm_allocated(),
        "button mount must not need the realm"
    );
    assert_eq!(bound_text(&app), "taps: 0");

    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    let button = app.dev_tool_get_element(column.children[1]).unwrap();
    let (bx, by) = button.absolute;
    let (bw, bh) = button.size;
    let cx = bx + bw / 2.0;
    let cy = by + bh / 2.0;

    for n in 1u64..=3 {
        app.click(cx, cy);
        app.wait_for_timeout(Duration::ZERO);
        assert!(
            !app.realm_allocated(),
            "click {n} must not construct the realm"
        );
        assert_eq!(
            bound_text(&app),
            format!("taps: {n}"),
            "click {n} drove the rut callback realm-free"
        );
    }
}

/// Scroll journey on a realm-free instance: the wheel path (gesture +
/// scroll subsystems, layout, paint) is pure Rust.
#[test]
fn rut_scroll_stays_realm_free() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(SCROLL_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        !app.realm_allocated(),
        "scroll mount must not need the realm"
    );

    let root = app.dev_tool_element_tree().unwrap();
    let root_col = app.dev_tool_get_element(root.children[0]).unwrap();
    let flexible = app.dev_tool_get_element(root_col.children[0]).unwrap();
    let scroller = app.dev_tool_get_element(flexible.children[0]).unwrap();
    let content = app.dev_tool_get_element(scroller.children[0]).unwrap();
    assert!(
        content.size.1 > scroller.size.1,
        "content overflows: content {:?} viewport {:?}",
        content.size,
        scroller.size
    );

    let cx = scroller.absolute.0 + scroller.size.0 / 2.0;
    let cy = scroller.absolute.1 + scroller.size.1 / 2.0;
    app.wheel(0.0, 120.0, cx, cy);
    app.wait_for_timeout(Duration::ZERO);

    let flexible2 = app.dev_tool_get_element(root_col.children[0]).unwrap();
    let scrolled = app.dev_tool_get_element(flexible2.children[0]).unwrap();
    let content2 = app.dev_tool_get_element(scrolled.children[0]).unwrap();
    assert!(
        content2.absolute.1 < content.absolute.1,
        "wheel scrolled the content up realm-free: {:?} -> {:?}",
        content.absolute.1,
        content2.absolute.1
    );
    assert!(
        !app.realm_allocated(),
        "wheel + scroll subsystems must not construct the realm"
    );
}

/// Reload journey on a realm-free instance: teardown + re-mount (with a
/// stop contract and a broken-reload in between) never allocate the realm.
#[test]
fn rut_reload_stays_realm_free() {
    let app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(BUTTON_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(bound_text(&app), "taps: 0");

    // A broken reload must NOT destroy the running tree (parse-first) —
    // and must not allocate the realm either.
    let err = app
        .load_rut_module("entry fn start() { let x: i32 = ; }")
        .unwrap_err();
    assert!(!err.to_string().is_empty(), "broken module fails loudly");
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        !app.realm_allocated(),
        "a broken rut reload must not construct the realm"
    );
    assert!(
        app.dev_tool_element_tree().is_some(),
        "the previous tree is still mounted"
    );

    // A good reload replaces the root — realm-free teardown + mount.
    const V2: &str = r#"
use tur::{ el_text, mount };

entry fn start() {
    mount(el_text("v2 root"));
}

entry fn stop() {
}
"#;
    app.load_rut_module(V2).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let root = app.dev_tool_element_tree().expect("v2 root mounted");
    assert_eq!(root.children.len(), 1);
    assert!(
        !app.realm_allocated(),
        "teardown + reload must not construct the realm"
    );
}
