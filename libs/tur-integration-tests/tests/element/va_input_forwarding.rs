//! VirtualAppView child keyboard/IME forwarding — keys are focus-routed.
//!
//! A focused Input inside a hosted child must receive key/IME events. The
//! chain: the child reports focus in/out (a `FocusChanged` host message →
//! `VirtualFocusEvent` into the parent's worker), the parent's focus
//! manager learns "focus sits inside this host element", and the parent's
//! key rail forwards `ShellEvent::Key` / `ShellEvent::Ime` into the child
//! untranslated (keys carry no position — focus-routing, not hit-testing).
//!
//! The child's controller text is read back through a deliberate probe
//! entry (`sync`) that mirrors it into a bound label on the child instance.

use std::rc::Rc;
use std::time::{Duration, Instant};

use tur_integration_tests::TurTestApp;

/// Read a keyed Text node's rendered content on an app facade (the rut
/// corpus's standard state probe, via the instance's own tree face).
fn label_text(app: &Rc<tur_engine::TurApp>, key: &str) -> Option<String> {
    let key = key.to_string();
    futures::executor::block_on(app.with_tree(move |tree, _focus| {
        let id = tree.query_element(&[key.as_str()])?;
        let node = tree.get_element(tur_engine::core::element::ElementNodeId::new(id.as_u64()))?;
        let element = node.element.as_ref()?;
        use tur_engine::builtin_plugins::text::TextElement;
        element.cast::<TextElement>().map(|c| {
            c.spans()
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>()
        })
    }))
    .flatten()
}

/// The child case: an Input (like the playground's password-input) plus a
/// bound label; the `sync` probe mirrors the controller text into the label
/// so the test can read it through the child facade.
const CHILD_SRC: &str = r#"
use tur::{ mount, rs_set_str, rs_source_str, st_put, st_take, tctrl_new, tctrl_text, undo_new };
use tur_kit::{ Column, Input, Text };


entry fn start() -> u64 {
    let ctrl = tctrl_new();
    st_put(7, ctrl);
    let label = rs_source_str("");
    let undo = undo_new();
    let input = Input().controller(ctrl).undo(undo).placeholder("type here").width_height(200.0, 32.0).query_key("child-input").build();
    let txt = Text().text_bound(label).query_key("child-text").build();
    mount(Column().child(input).child(txt).build());
    return label;
}

// Test probe: mirror the controller text into the bound label (entries
// return nothing — the label is the read-back channel). The label atom
// rides the entry arg (`start` returns it).
entry fn sync(label: u64, _b: f64) {
    let ctrl = st_take(7);
    st_put(7, ctrl);
    rs_set_str(label, tctrl_text(ctrl));
}
"#;

fn parent_module(child_src: &str) -> String {
    // Splice the child source through a rut string literal — escape the
    // two characters a rut string cares about (`\` and `"`); newlines go
    // out as `\n` escapes.
    let escaped = child_src
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!(
        r#"
use tur::{{ mount, va_controller, va_create_source }};
use tur_kit::{{ VirtualApp }};


entry fn start() {{
    let src = va_create_source("{escaped}");
    let ctrl = va_controller(src);
    let host = VirtualApp().controller(ctrl).width_height(400.0, 200.0).build();
    mount(host);
}}
"#
    )
}

/// Boot parent + child; returns the parent app and the child facade once
/// the child's tree has mounted (module loaded + `start` ran + first
/// layout). `wait_key` is the child element the mount poll waits for.
fn setup_with_child_src(child_src: &str, wait_key: &str) -> (TurTestApp, Rc<tur_engine::TurApp>) {
    let wait_key = wait_key.to_string();
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(&parent_module(child_src)).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // The child compiles + boots on the virtual-pool worker (real time).
    // Poll until its wait_key element exists in its own tree.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let children = app.app().virtual_apps();
        if let Some(child) = children.first() {
            let key = wait_key.clone();
            let mounted = futures::executor::block_on(
                child.with_tree(move |tree, _focus| tree.query_element(&[key.as_str()]).is_some()),
            )
            .unwrap_or(false);
            if mounted {
                break (app, child.clone());
            }
        }
        assert!(
            Instant::now() < deadline,
            "the child instance never mounted its input"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn setup() -> (TurTestApp, Rc<tur_engine::TurApp>) {
    setup_with_child_src(CHILD_SRC, "child-input")
}

/// The focused element id on the parent's focus manager (the key-routing
/// gate) — `None` when nothing is focused.
fn parent_focused(app: &TurTestApp) -> Option<u64> {
    futures::executor::block_on(
        app.app()
            .with_tree(|_tree, focus| focus.focused().map(|id| u64::from(id))),
    )
    .flatten()
}

/// The parent-space element id of the (single) `tur_virtual_app` host.
fn host_id(app: &TurTestApp) -> Option<u64> {
    futures::executor::block_on(app.app().with_tree(|tree, _focus| {
        let root = tree.root_element_id()?;
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if let Some(node) = tree.get_element(id) {
                if let Some(el) = node.element.as_ref()
                    && el.type_name() == "tur_virtual_app"
                {
                    return Some(u64::from(id));
                }
                stack.extend(tree.children_of_element(id));
            }
        }
        None
    }))
    .flatten()
}

/// The child controller's current text (via the sync probe + label read).
fn child_text(child: &Rc<tur_engine::TurApp>) -> String {
    let label_atom = futures::executor::block_on(child.rut_start_answer());
    futures::executor::block_on(child.call_rut_entry("sync", label_atom, 0.0)).expect("sync probe");
    label_text(child, "child-text").unwrap_or_default()
}

/// Keys typed at the parent reach the focused Input inside the child: the
/// click focuses the child input (the pointer rail already forwards), the
/// child reports focus, and the parent's key rail forwards into the child.
#[test]
fn va_child_receives_key_events_when_its_input_is_focused() {
    let (mut app, child) = setup();
    let host = host_id(&app).expect("host element");

    // Click the child input — the child lays it out at (0..200, 0..32) in
    // the host, and the host sits at the parent origin.
    app.click(100.0, 16.0);
    // The click focuses the input inside the child (the pointer rail), and
    // the child's focus report arms the parent's routing gate — wait for
    // that gate before typing (keys are focus-routed).
    let armed = app.wait_for(|_| parent_focused(&app) == Some(host));
    assert!(
        armed,
        "the parent's focus manager should hold the child host"
    );

    // Typed keys flow: parent key rail → focused child host → child input.
    app.send_key("a");
    app.send_key("b");
    let grew = app.wait_for(|_| child_text(&child) == "ab");
    assert!(grew, "child input should have received the typed keys");
}

/// IME composition rides the same focus-routed rail: a composition started,
/// updated, and ended against the parent commits its text into the focused
/// child input.
#[test]
fn va_child_receives_ime_composition_end() {
    let (mut app, child) = setup();
    let host = host_id(&app).expect("host element");

    app.click(100.0, 16.0);
    let armed = app.wait_for(|_| parent_focused(&app) == Some(host));
    assert!(
        armed,
        "the parent's focus manager should hold the child host"
    );

    use tur_engine::core::shell::ImeEvent;
    app.send_ime(ImeEvent::CompositionStart);
    app.send_ime(ImeEvent::CompositionUpdate {
        text: "héllo".to_string(),
        cursor: None,
    });
    app.send_ime(ImeEvent::CompositionEnd {
        text: "héllo".to_string(),
    });
    let grew = app.wait_for(|_| child_text(&child) == "héllo");
    assert!(
        grew,
        "child input should have received the composition text"
    );
}

/// Focus loss: clicking outside the host (parent background) blurs the
/// child, and later keys no longer reach it.
#[test]
fn va_child_loses_focus_when_the_parent_clicks_away() {
    let (mut app, child) = setup();
    let host = host_id(&app).expect("host element");

    app.click(100.0, 16.0);
    let armed = app.wait_for(|_| parent_focused(&app) == Some(host));
    assert!(
        armed,
        "the parent's focus manager should hold the child host"
    );
    app.send_key("a");
    let grew = app.wait_for(|_| child_text(&child) == "a");
    assert!(grew, "the typed key reached the focused child input");

    // Click the parent background below the 200px-tall host.
    app.click(300.0, 280.0);
    // The click blurs the host (the gesture sweep) → ClearFocus into the
    // child. Wait for the gate to release before typing again.
    let blurred = app.wait_for(|_| parent_focused(&app).is_none());
    assert!(blurred, "the parent should have taken focus back");
    app.send_key("c");
    app.wait_for_timeout(Duration::from_millis(32));
    assert_eq!(
        child_text(&child),
        "a",
        "keys must stop reaching the child after the parent regains focus"
    );
}

// ===========================================================================
// Wheel forwarding — position-routed like pointer input. A `ShellEvent::
// Wheel` over the host element must reach the CHILD (translated into
// child-local coordinates) and scroll the child's scrollable; a wheel
// outside the host rect must not.
// ===========================================================================

/// The child case: a ScrollView whose content (1200px in a 200px-tall
/// host) overflows, keyed for the offset probe.
const WHEEL_CHILD_SRC: &str = r#"
use tur::{ AXIS_VERTICAL, mount };
use tur_kit::{ Column, Container, ScrollView, Text };


entry fn start() -> u64 {
    let rows = Column();
    let rows = rows.child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r0").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r1").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r2").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r3").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r4").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r5").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r6").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r7").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r8").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r9").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r10").font_size(12.0).build()).build())
        .child(Container().width_height(400.0, 100.0).color(0x0F172AFFu64).child(Text().text("r11").font_size(12.0).build()).build());
    mount(ScrollView().axis(AXIS_VERTICAL).child(rows.build()).query_key("child-scroll").build());
    return 0;
}
"#;

/// The child scroll view's current offset (the wheel-forward readback).
fn child_scroll_offset(child: &Rc<tur_engine::TurApp>) -> f64 {
    futures::executor::block_on(child.with_tree(|tree, _focus| {
        let id = tree.query_element(&["child-scroll"])?;
        let node = tree.get_element(tur_engine::core::element::ElementNodeId::new(id.as_u64()))?;
        let element = node.element.as_ref()?;
        use tur_engine::builtin_plugins::scroll::ScrollViewElement;
        element
            .cast::<ScrollViewElement>()
            .map(|sv| sv.scroll_offset())
    }))
    .flatten()
    .unwrap_or(f64::NAN)
}

#[test]
fn va_child_wheel_over_the_host_scrolls_the_child_scrollable() {
    let (mut app, child) = setup_with_child_src(WHEEL_CHILD_SRC, "child-scroll");

    // Wheel over the host (400×200 at the parent origin) — the child-local
    // point lands on the child's scroll view, which fills its viewport.
    app.wheel(0.0, 150.0, 200.0, 100.0);
    let scrolled = app.wait_for(|_| child_scroll_offset(&child) > 100.0);
    assert!(
        scrolled,
        "the wheel over the host must scroll the child's scroll view, got {}",
        child_scroll_offset(&child)
    );

    // Wheel OUTSIDE the host (below its 200px bottom) — the parent keeps
    // the event; the child's offset is unchanged.
    let before = child_scroll_offset(&child);
    app.wheel(0.0, 150.0, 200.0, 280.0);
    app.wait_for_timeout(Duration::from_millis(32));
    assert_eq!(
        child_scroll_offset(&child),
        before,
        "a wheel outside the host rect must not reach the child"
    );
}
