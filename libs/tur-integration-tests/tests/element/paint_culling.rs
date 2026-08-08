//! Paint-walk culling: nodes whose laid-out bbox falls entirely outside the
//! active clip (explicit element clip OR the viewport seed) are skipped before
//! their element paint body runs — so they emit no `RenderCommand::Paint`.
//!
//! These tests verify both directions (off-screen → culled; scrolled into view
//! → painted again) using a `ScrollView` whose children are positioned by
//! layout offset (which IS reflected in each child's `absolute` affine, so the
//! cull check is correct).

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use tur_engine::core::element::ElementNodeId;
use tur_engine::core::render::{RenderCommand, Renderer};
use tur_integration_tests::TurTestApp;

/// Renderer that stashes each frame's command batch so the test can inspect
/// which nodes produced a `Paint` (and, by omission, which were culled).
struct RecordingRenderer {
    last: Rc<RefCell<Vec<RenderCommand>>>,
}

impl Renderer for RecordingRenderer {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        *self.last.borrow_mut() = commands.to_vec();
    }
}

/// Node ids that produced at least one `Paint` this frame.
fn painted_ids(cmds: &[RenderCommand]) -> HashSet<ElementNodeId> {
    cmds.iter()
        .filter_map(|c| match c {
            RenderCommand::Paint { id, .. } => Some(*id),
            _ => None,
        })
        .collect()
}

/// Mount a `ScrollView > Column > 6 Containers` (each 100px → 600px of content
/// in a 300px-tall viewport) and return the 6 container node ids in order.
fn mount_and_collect_ids(app: &mut TurTestApp) -> Vec<ElementNodeId> {
    app.eval_module_source(
        r#"
        import { render, ScrollView, Column, Container, createColor } from "tur:std";
        const kids = [];
        for (let i = 0; i < 6; i++) {
            kids.push(Container({
                height: 100,
                color: createColor(255, 0, 0, 255),
                queryKey: ["item", i],
            }));
        }
        render(ScrollView({ queryKey: ["scroll"], child: Column({ children: kids }) }));
    "#,
    )
    .expect("mount");

    app.render();

    let tree = app.element_tree();
    let root = tree.root_element().expect("root");
    // root → ScrollView (first element child).
    let sv_id = tree
        .children_of_element(root.id)
        .into_iter()
        .next()
        .expect("scrollview child");
    // ScrollView → Column.
    let col_id = tree
        .children_of_element(sv_id)
        .into_iter()
        .next()
        .expect("column child");
    // Column → 6 Containers.
    let containers = tree.children_of_element(col_id);
    assert_eq!(
        containers.len(),
        6,
        "expected 6 containers, got {containers:?}"
    );
    containers
}

#[test]
fn offscreen_scroll_children_are_culled() {
    let last: Rc<RefCell<Vec<RenderCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let mut app = TurTestApp::new_with_renderer(
        400.0,
        300.0,
        Box::new(RecordingRenderer { last: last.clone() }),
    )
    .expect("app");

    let containers = mount_and_collect_ids(&mut app);

    // Viewport is 300px tall; content is 600px. With scroll = 0 the first
    // three containers (y 0..300) are on-screen; the last three (y 300..600)
    // are fully below the viewport and must be culled.
    let painted = painted_ids(&last.borrow());
    assert!(
        painted.contains(&containers[0]),
        "container 0 (top, on-screen) must be painted"
    );
    assert!(
        painted.contains(&containers[2]),
        "container 2 (on-screen) must be painted"
    );
    assert!(
        !painted.contains(&containers[4]),
        "container 4 (off-screen) must be culled"
    );
    assert!(
        !painted.contains(&containers[5]),
        "container 5 (off-screen) must be culled"
    );
    // At least two of the six were culled.
    let painted_count = containers.iter().filter(|id| painted.contains(id)).count();
    assert!(
        painted_count <= 4,
        "expected at most 4 painted (some culled), got {painted_count}"
    );
}

#[test]
fn scrolled_in_children_become_painted() {
    let last: Rc<RefCell<Vec<RenderCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let mut app = TurTestApp::new_with_renderer(
        400.0,
        300.0,
        Box::new(RecordingRenderer { last: last.clone() }),
    )
    .expect("app");

    let containers = mount_and_collect_ids(&mut app);

    // Scroll to the bottom (delta clamps to max extent = 600 - 300 = 300px).
    // Pointer (200, 150) is inside the 400×300 scroll viewport.
    app.wheel(0.0, 500.0, 200.0, 150.0);
    app.render();

    let painted = painted_ids(&last.borrow());
    // After scrolling 300px down: container 0 sits at y -300..-200 (above the
    // viewport) → culled; container 5 sits at y 200..300 (visible) → painted.
    assert!(
        !painted.contains(&containers[0]),
        "container 0 (scrolled off top) must be culled"
    );
    assert!(
        !painted.contains(&containers[1]),
        "container 1 (scrolled off top) must be culled"
    );
    assert!(
        painted.contains(&containers[5]),
        "container 5 (scrolled into view) must be painted"
    );
    assert!(
        painted.contains(&containers[3]),
        "container 3 (scrolled into view) must be painted"
    );
}

#[test]
fn no_clip_means_no_culling() {
    // Sanity: with no ScrollView/clip, content beyond the viewport is still
    // culled by the viewport seed alone (a node fully outside the screen is
    // invisible anyway). A node INSIDE the viewport is always painted.
    let last: Rc<RefCell<Vec<RenderCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let mut app = TurTestApp::new_with_renderer(
        400.0,
        300.0,
        Box::new(RecordingRenderer { last: last.clone() }),
    )
    .expect("app");

    app.eval_module_source(
        r#"
        import { render, Container, createColor } from "tur:std";
        render(Container({
            width: 100,
            height: 100,
            color: createColor(0, 128, 255, 255),
            queryKey: ["onscreen"],
        }));
    "#,
    )
    .expect("mount");
    app.render();

    let tree = app.element_tree();
    let root = tree.root_element().expect("root");
    let child = tree
        .children_of_element(root.id)
        .into_iter()
        .next()
        .expect("child");
    let painted = painted_ids(&last.borrow());
    assert!(
        painted.contains(&child),
        "an on-screen node with no explicit clip must still be painted"
    );
}
