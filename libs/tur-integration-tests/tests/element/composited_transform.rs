use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::core::elements::NodeTreeData;
use tur_integration_tests::TurTestApp;

/// Depth-first walk; return the first node whose element `kind()` matches.
fn find_by_kind(tree: &NodeTreeData, id: ElementNodeId, kind: &ElementKind) -> Option<ElementNodeId> {
    let node = tree.get_element(id)?;
    if node.element.as_ref().map(|e| e.kind()) == Some(kind.clone()) {
        return Some(id);
    }
    for c in &node.children {
        if let Some(found) = find_by_kind(tree, ElementNodeId::new(c.as_u64()), kind) {
            return Some(found);
        }
    }
    None
}

fn abs_top_left(app: &TurTestApp, id: ElementNodeId) -> (f64, f64) {
    let b = app.get_element_absolute_bounds(id).unwrap();
    (b.left, b.top)
}

/// Default TopLeft/TopLeft anchors + zero targetOffset → the follower's
/// top-left lands exactly on the target's top-left (100, 80).
#[test]
fn follower_lands_on_target() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("composited-transform-basic").unwrap();
    app.render();

    let (target_id, follower_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let target = find_by_kind(
            &tree,
            root.id,
            &ElementKind::new("tur_composited_transform_target"),
        )
        .expect("target mounted");
        let follower = find_by_kind(
            &tree,
            root.id,
            &ElementKind::new("tur_composited_transform_follower"),
        )
        .expect("follower mounted");
        (target, follower)
    };

    let (tx, ty) = abs_top_left(&app, target_id);
    let (fx, fy) = abs_top_left(&app, follower_id);
    assert!(
        (tx - 100.0).abs() < 1e-3 && (ty - 80.0).abs() < 1e-3,
        "target absolute origin should be (100, 80) — got ({tx}, {ty})"
    );
    assert!(
        (fx - tx).abs() < 1e-3 && (fy - ty).abs() < 1e-3,
        "follower should land on target ({tx}, {ty}) — got ({fx}, {fy})"
    );
}

/// The follower composes the target's full world affine (ancestor `Transform`
/// paint-only translate included). Layout places the Transform at (20, 20); the
/// translate (50, 10) is paint-only → the follower must land at (70, 30), not
/// the layout position (20, 20).
#[test]
fn follower_tracks_target_through_transform() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("composited-transform-transform").unwrap();
    app.render();

    let (target_id, follower_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let target = find_by_kind(
            &tree,
            root.id,
            &ElementKind::new("tur_composited_transform_target"),
        )
        .expect("target mounted");
        let follower = find_by_kind(
            &tree,
            root.id,
            &ElementKind::new("tur_composited_transform_follower"),
        )
        .expect("follower mounted");
        (target, follower)
    };

    // The target's absolute *layout* origin is (20, 20) (Positioned), but its
    // painted position — and thus the follower — is shifted by the Transform's
    // translate to (70, 30).
    let (fx, fy) = abs_top_left(&app, follower_id);
    assert!(
        (fx - 70.0).abs() < 1e-3 && (fy - 30.0).abs() < 1e-3,
        "follower should land at the transform-shifted target (70, 30) — got ({fx}, {fy})"
    );
    let _ = target_id;
}

/// `targetAnchor` is reactive (`Val<Alignment>`): flipping the source from
/// `TopLeft` to `BottomRight` (via a button click) must relocate the follower
/// from the target's top-left to its bottom-right on the next frame.
#[test]
fn follower_tracks_reactive_anchor_change() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("composited-transform-reactive-anchor").unwrap();
    app.render();

    let follower_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        find_by_kind(
            &tree,
            root.id,
            &ElementKind::new("tur_composited_transform_follower"),
        )
        .expect("follower mounted")
    };

    // Initially TopLeft/TopLeft, zero offset → follower at target's top-left
    // (100, 80).
    let (fx0, fy0) = abs_top_left(&app, follower_id);
    assert!(
        (fx0 - 100.0).abs() < 1e-3 && (fy0 - 80.0).abs() < 1e-3,
        "follower should start at target top-left (100, 80) — got ({fx0}, {fy0})"
    );

    // Click the button at (30, 550) → sets targetAnchor to BottomRight.
    app.click(30.0, 550.0);

    // Target at (100, 80), size 60×40 → bottom-right at (160, 120). The
    // follower's own anchor is still TopLeft, so its top-left lands there.
    let (fx1, fy1) = abs_top_left(&app, follower_id);
    assert!(
        (fx1 - 160.0).abs() < 1e-3 && (fy1 - 120.0).abs() < 1e-3,
        "follower should relocate to target bottom-right (160, 120) after anchor change — got ({fx1}, {fy1})"
    );
}

/// The follower continuously tracks the target: scrolling the target's
/// ScrollView shifts the target's absolute position, and the follower must
/// move with it (follower origin stays equal to the target origin).
#[test]
fn follower_tracks_target_through_scroll() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("composited-transform-scroll").unwrap();
    app.render();

    let (target_id, follower_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let target = find_by_kind(
            &tree,
            root.id,
            &ElementKind::new("tur_composited_transform_target"),
        )
        .expect("target mounted");
        let follower = find_by_kind(
            &tree,
            root.id,
            &ElementKind::new("tur_composited_transform_follower"),
        )
        .expect("follower mounted");
        (target, follower)
    };

    let (tx0, ty0) = abs_top_left(&app, target_id);
    let (fx0, fy0) = abs_top_left(&app, follower_id);
    assert!(
        (fx0 - tx0).abs() < 1e-3 && (fy0 - ty0).abs() < 1e-3,
        "follower should start on target ({tx0}, {ty0}) — got ({fx0}, {fy0})"
    );

    // Scroll down — the target's absolute y must decrease, and the follower
    // must follow.
    app.wheel(0.0, 60.0, 50.0, 50.0);

    let (tx1, ty1) = abs_top_left(&app, target_id);
    let (fx1, fy1) = abs_top_left(&app, follower_id);
    assert!(
        (ty1 - ty0).abs() > 1e-3,
        "scroll should have moved the target (was {ty0}, now {ty1})"
    );
    assert!(
        (fx1 - tx1).abs() < 1e-3 && (fy1 - ty1).abs() < 1e-3,
        "follower should track target after scroll ({tx1}, {ty1}) — got ({fx1}, {fy1})"
    );
}
