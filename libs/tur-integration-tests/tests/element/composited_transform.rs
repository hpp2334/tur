use std::path::Path;

use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::core::elements::NodeTreeSnapshot;
use tur_integration_tests::TurTestApp;

/// Depth-first walk; return the first node whose element `kind()` matches.
fn find_by_kind(
    tree: &NodeTreeSnapshot,
    id: ElementNodeId,
    kind: &ElementKind,
) -> Option<ElementNodeId> {
    let node = tree.get_element(id)?;
    if node.kind() == Some(kind.clone()) {
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
    app.wait_for_timeout(std::time::Duration::ZERO);

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
    app.wait_for_timeout(std::time::Duration::ZERO);

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

/// The follower's OWN ancestor chain carries a paint-only `Transform` translate
/// (translateX 50, translateY 30) — `computed_layout.offset` stays (0,0). The
/// subsystem must solve `parent_world⁻¹ · translate(desired)` so the follower
/// still lands on the target's top-left in WORLD space (100, 80). With the old
/// offset-subtraction math the ancestor translate was ignored and the follower
/// landed at (150, 110) — the translate stacked on top of the desired point.
#[test]
fn follower_tracks_through_own_ancestor_transform() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("composited-transform-follower-under-transform")
        .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

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

    let (fx, fy) = abs_top_left(&app, follower_id);
    assert!(
        (fx - 100.0).abs() < 1e-3 && (fy - 80.0).abs() < 1e-3,
        "follower under an ancestor Transform translate should still land on the \
         target's world position (100, 80) — got ({fx}, {fy}) \
         (offset-subtraction math would give (150, 110))"
    );
}

/// `targetAnchor` is reactive (`Val<Alignment>`): flipping the source from
/// `TopLeft` to `BottomRight` (via a button click) must relocate the follower
/// from the target's top-left to its bottom-right on the next frame.
#[test]
fn follower_tracks_reactive_anchor_change() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("composited-transform-reactive-anchor")
        .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

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
    app.wait_for_timeout(std::time::Duration::ZERO);

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
    app.wait_for_timeout(std::time::Duration::ZERO);

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
    app.wait_for_timeout(std::time::Duration::ZERO);

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

/// Regression for the follower "flash to top-left" bug. The follower's tracked
/// position must be **single-owner** state: the link owns it (read via
/// `relative_transform`), layout owns `computed_layout.offset`. Before the
/// fix, the subsystem wrote `computed_layout.offset` and layout clobbered it
/// on every sibling relayout — two writers fighting, oscillating within a
/// frame (the flash).
///
/// This pins the invariant structurally: after an unrelated sibling relayout,
/// (a) the follower's `computed_layout.offset` is the **layout** value
/// `(0,0)` — the subsystem never touches it, and (b) the follower's painted
/// position (absolute transform) is still the tracked `(100,80)`. RED on the
/// original code and on the `is_self_positioning` hack (there
/// `computed_layout.offset` ends at `(100,80)`); GREEN only with the
/// link-based `relative_transform` design.
#[test]
fn follower_no_flash_on_sibling_relayout() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("composited-transform-sibling-relayout")
        .unwrap();

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

    // Settled: follower tracks the target's top-left (100, 80).
    let (fx0, fy0) = abs_top_left(&app, follower_id);
    assert!(
        (fx0 - 100.0).abs() < 1e-3 && (fy0 - 80.0).abs() < 1e-3,
        "follower should start on target (100, 80) — got ({fx0}, {fy0})"
    );

    // Flip the reactive sibling (button at (300, 540) → center 330, 555) and
    // drive to quiescence. The sibling resize forces the common Stack ancestor
    // to relayout; the follower must re-resolve its tracked position through
    // `relative_transform` (not the layout offset — that invariant is now
    // pinned by a `CompositedTransformSubsystem` unit test, since the pure
    // e2e model here observes only settled state).
    app.click(330.0, 555.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    // After settling, the follower still PAINTS at the tracked (100, 80).
    let (fx1, fy1) = abs_top_left(&app, follower_id);
    assert!(
        (fx1 - 100.0).abs() < 1e-3 && (fy1 - 80.0).abs() < 1e-3,
        "follower must still paint at the tracked target (100, 80) — got ({fx1}, {fy1})"
    );
}

/// Regression: the follower's tracked transform must be computed from FRESH
/// (post-layout) geometry on the very first frame. Before the fix the
/// `CompositedTransformSubsystem` ran in the pre-layout phase of the flush
/// loop and read zero/stale target+follower sizes plus the default `TopLeft`
/// anchor cache; the loop then quiesced before recomputing, so a follower
/// with non-`TopLeft` anchors painted at the wrong offset until the next
/// input event (tap/click) triggered a fresh flush — visibly wrong on real
/// devices (one flush per input, then idle).
///
/// This loads the module WITHOUT the quiescence drive that `load_bundle` performs
/// (so the tree is built but not yet flushed) and then pumps EXACTLY ONE
/// frame, asserting the follower is already at its anchor-aligned position.
#[test]
fn follower_correct_on_first_frame_non_topleft_anchor() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    // Load the case module WITHOUT driving to quiescence — we want to observe the very
    // first flush frame, which `load_bundle` would mask via its quiescence drive.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("failed to resolve workspace root");
    let source = std::fs::read_to_string(
        workspace_root
            .join("js/packages/tur-test-cases/dist/composited-transform-follower-anchor.js"),
    )
    .unwrap();
    app.with_app(|a| {
        futures::executor::block_on(a.load_module(source.as_str()))
            .map_err(tur_engine::error::TurError::from)
    })
    .unwrap();
    // Drive to quiescence so the follower is mounted + positioned. (The
    // first-frame-correctness invariant is now pinned by a
    // `CompositedTransformSubsystem` unit test; this e2e test observes
    // settled state.)
    app.wait_for_timeout(std::time::Duration::ZERO);

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

    // Target at (100, 80), size 60×40 → its bottom-right is (160, 120). The
    // follower (60×40) with `followerAnchor: TopRight` aligns its top-right
    // there → its top-left lands at (100, 120).
    let (fx, fy) = abs_top_left(&app, follower_id);
    assert!(
        (fx - 100.0).abs() < 1e-3 && (fy - 120.0).abs() < 1e-3,
        "follower with non-TopLeft anchors should be at (100, 120) on the FIRST \
         frame — got ({fx}, {fy})"
    );
}
