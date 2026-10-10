//! A programmatic controller write must refresh the **mounted** editable.
//! `tctrl_set_text` (the playground's case-selection path) updates the
//! shared controller's spans + revision, and the mounted Input's next
//! painted layout must follow: the revision feeds the element's layout
//! memo key, so the next layout pass re-shapes — provided one runs. The
//! controller is an opaque `Rc` binding invisible to the reactive build
//! dedup, so the mutating rows must mark the mounted node dirty and
//! request a frame (the paste path's law — `tree.mark_dirty`).
//!
//! Regresses the playground defect where selecting a case flipped the
//! status line to `ok` while the editor kept painting its placeholder
//! forever (the model held the source; the pixels never followed).

use tur_engine::core::elements::{DevNodeData, TraceValue};
use tur_integration_tests::TurTestApp;

const EDITOR_RUT: &str = r#"
use tur_kit::handles::{ TextCtrl, mount, text_ctrl };
use tur_kit::text::input::{ Input };

struct EditorCx {
    ctrl: TextCtrl,
}

fn start() -> EditorCx {
    let ctrl = text_ctrl();
    let input = Input().controller(ctrl).width_height(200.0, 44.0).query_key("editor").build();
    mount(input);
    return EditorCx { ctrl: ctrl };
}

entry fn entry_start() -> opaque {
    let cx = start();
    return opaque(cx);
}

fn editor_cx(cx: opaque) -> EditorCx {
    let c = opaque.downcast<EditorCx>(cx);
    if (c == nil) {
        panic("editor fixture: cx is not an EditorCx");
    }
    return c;
}

// The playground's `case_tap` shape: a programmatic write into the
// controller the mounted Input was built with.
entry fn set_text(cx: opaque) {
    editor_cx(cx).ctrl.set_text("hello from the row");
}
"#;

/// Depth-first search for the editable node under `node`.
fn find_editable(app: &TurTestApp, node: DevNodeData) -> Option<DevNodeData> {
    if node.name == "tur_editable_text" {
        return Some(node);
    }
    for child in node.children {
        let found = app.dev_tool_get_element(child)?;
        if let Some(hit) = find_editable(app, found) {
            return Some(hit);
        }
    }
    None
}

fn metric(node: &DevNodeData, key: &str) -> f64 {
    node.layout_extra
        .iter()
        .find(|(k, _)| *k == key)
        .and_then(|(_, v)| match v {
            TraceValue::Num(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(-1.0)
}

#[test]
fn tctrl_set_text_refreshes_the_mounted_editable() {
    let app = TurTestApp::new(300.0, 120.0).expect("app builds");
    app.load_rut_module(EDITOR_RUT).expect("module loads");
    let cx = app.call_rut_entry_opaque("entry_start").expect("boot");
    app.pump();

    // Locate the mounted editable and record its pre-write painted
    // metrics (the empty-content shape).
    let root = app
        .dev_tool_element_tree()
        .expect("dev-tool root after boot");
    let before = find_editable(&app, root).expect("editable mounted");
    let (before_w, before_shapes) = (
        metric(&before, "layoutWidth"),
        metric(&before, "shapeCount"),
    );

    // The programmatic write — then drive the frame that must repaint it.
    app.call_rut_entry_cx("set_text", cx).expect("entry runs");
    app.pump();

    let root = app
        .dev_tool_element_tree()
        .expect("dev-tool root after write");
    let after = find_editable(&app, root).expect("editable still mounted");
    let (after_w, after_shapes) = (metric(&after, "layoutWidth"), metric(&after, "shapeCount"));

    assert!(
        after_w != before_w,
        "the editable's shaped width must follow a programmatic controller \
         write (before: {before_w}, after: {after_w} — frozen at the \
         pre-write shape means the mounted view never refreshed)"
    );
    assert!(
        after_shapes > before_shapes,
        "the re-shape counter must advance (before: {before_shapes}, after: {after_shapes})"
    );
}
