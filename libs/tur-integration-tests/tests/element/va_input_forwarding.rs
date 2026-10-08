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
//! entry (`sync`) that answers the controller's live text (the
//! context-crossing contract: the probe takes the held `cx` and
//! downcasts; the mirror label is gone — the answer IS the readback).

use std::rc::Rc;
use std::time::{Duration, Instant};

use tur_integration_tests::TurTestApp;

/// The child case: an Input (like the playground's password-input); the
/// `sync` probe answers the controller's live text so the test can read it
/// through the child facade. The controller rides the answered context
/// (`AppContext`) — no stash, no raw rows.
const CHILD_SRC: &str = r#"
use tur_kit::{ Column, Input, TextCtrl, UndoCtrl, mount, text_ctrl, undo_ctrl };

struct ChildCx {
    ctrl: TextCtrl,
    undo: UndoCtrl,
}

fn start() -> ChildCx {
    let ctrl = text_ctrl();
    let undo = undo_ctrl();
    let input = Input().controller(ctrl).undo(undo).placeholder("type here").width_height(200.0, 32.0).query_key("child-input").build();
    mount(Column().child(input).build());
    return ChildCx { ctrl: ctrl, undo: undo };
}

entry fn entry_start() -> opaque {
    let cx = start();
    return opaque(cx);
}

fn child_cx(cx: opaque) -> ChildCx {
    let c = opaque.downcast<ChildCx>(cx);
    if (c == nil) {
        panic("va child fixture: cx is not a ChildCx");
    }
    return c;
}

// Test probe: the answered str is the controller's live text (entries
// answer — the readback crosses the entry lane's answer slot).
entry fn sync(cx: opaque) -> str {
    return child_cx(cx).ctrl.text();
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
use tur_kit::{{ VirtualApp, mount, virtual_app_controller, virtual_app_source }};

entry fn start() {{
    let src = virtual_app_source("{escaped}");
    let ctrl = virtual_app_controller(src);
    let host = VirtualApp().controller(ctrl).width_height(400.0, 200.0).build();
    mount(host);
}}
"#
    )
}

/// Boot parent + child; returns the parent app, the child facade, and the
/// child's held context token (None for an eagerly-booted child) once the
/// child's tree has mounted (module loaded + boot + first layout).
/// `wait_key` is the child element the mount poll waits for.
fn setup_with_child_src(
    child_src: &str,
    wait_key: &str,
) -> (
    TurTestApp,
    Rc<tur_engine::TurApp>,
    Option<u64>,
) {
    let wait_key = wait_key.to_string();
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(&parent_module(child_src)).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // The child compiles + boots on the virtual-pool worker (real time).
    // The child's boot is LAZY (the fixture contract: no `start` export —
    // the embedder's `entry_start` probe runs it), so poll for the
    // facade, boot it ONCE, then wait for its wait_key element.
    let deadline = Instant::now() + Duration::from_secs(10);
    let child = loop {
        if let Some(child) = app.app().virtual_apps().first() {
            break child.clone();
        }
        assert!(
            Instant::now() < deadline,
            "the child instance never spawned"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    // The contract-shaped children boot LAZY (no `start` export — the
    // embedder's `entry_start` probe runs it and answers the context);
    // children that keep a plain `entry fn start()` booted eagerly at
    // load and hold no context.
    let answer = futures::executor::block_on(child.call_rut_entry_opaque("entry_start"));
    let cx = match answer {
        Ok(a) => Some(
            a.opaque_token()
                .expect("entry_start answers the child's context"),
        ),
        // "no export `entry_start`" = the eager-boot shape.
        Err(_) => None,
    };
    loop {
        let key = wait_key.clone();
        let mounted = futures::executor::block_on(
            child.with_tree(move |tree, _focus| tree.query_element(&[key.as_str()]).is_some()),
        )
        .unwrap_or(false);
        if mounted {
            break (app, child, cx);
        }
        assert!(
            Instant::now() < deadline,
            "the child instance never mounted its input"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn setup() -> (TurTestApp, Rc<tur_engine::TurApp>, u64) {
    let (app, child, cx) = setup_with_child_src(CHILD_SRC, "child-input");
    (app, child, cx.expect("the contract child holds a context"))
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

/// The child controller's current text (the `sync` probe's answered str
/// over the held context).
fn child_text(child: &Rc<tur_engine::TurApp>, cx: u64) -> String {
    futures::executor::block_on(child.call_rut_entry_cx_str("sync", cx))
        .expect("sync probe")
}

/// Keys typed at the parent reach the focused Input inside the child: the
/// click focuses the child input (the pointer rail already forwards), the
/// child reports focus, and the parent's key rail forwards into the child.
#[test]
fn va_child_receives_key_events_when_its_input_is_focused() {
    let (mut app, child, cx) = setup();
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
    let grew = app.wait_for(|_| child_text(&child, cx) == "ab");
    assert!(grew, "child input should have received the typed keys");
}

/// IME composition rides the same focus-routed rail: a composition started,
/// updated, and ended against the parent commits its text into the focused
/// child input.
#[test]
fn va_child_receives_ime_composition_end() {
    let (mut app, child, cx) = setup();
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
    let grew = app.wait_for(|_| child_text(&child, cx) == "héllo");
    assert!(
        grew,
        "child input should have received the composition text"
    );
}

/// Focus loss: clicking outside the host (parent background) blurs the
/// child, and later keys no longer reach it.
#[test]
fn va_child_loses_focus_when_the_parent_clicks_away() {
    let (mut app, child, cx) = setup();
    let host = host_id(&app).expect("host element");

    app.click(100.0, 16.0);
    let armed = app.wait_for(|_| parent_focused(&app) == Some(host));
    assert!(
        armed,
        "the parent's focus manager should hold the child host"
    );
    app.send_key("a");
    let grew = app.wait_for(|_| child_text(&child, cx) == "a");
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
        child_text(&child, cx),
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

use tur_kit::{ Axis, Column, Container, MutationCtx, Readable, ScrollView, Source, Text, TextCtrl, UndoCtrl, mount, source };

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
    mount(ScrollView().axis(Axis.Vertical).child(rows.build()).query_key("child-scroll").build());
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
    let (mut app, child, _cx) = setup_with_child_src(WHEEL_CHILD_SRC, "child-scroll");

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
