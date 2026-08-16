use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

#[test]
fn image_with_explicit_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("image-basic").unwrap();

    let image_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let container = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        assert_eq!(container.kind().unwrap(), ElementKind::new("tur_container"));
        assert_eq!(container.children.len(), 1);

        let image = tree
            .get_element(ElementNodeId::new(container.children[0].as_u64()))
            .unwrap();
        assert_eq!(image.kind().unwrap(), ElementKind::new("tur_image"));
        assert_eq!(image.children.len(), 0);
        image.id
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();
    let image_node = rt.get_element(image_id).unwrap();
    assert_eq!(image_node.computed_layout.size.width, 200.0);
    assert_eq!(image_node.computed_layout.size.height, 100.0);
}

/// Phase-4 image ownership split: the worker keeps only metadata (sizes);
/// the decoded pixel `Blob` ships to main directly from the
/// `createImageResource` bridge (one `HostMsg::UploadImage` per decode,
/// via the shared `main_tx`). Main retains the full `ImageResource` (for
/// context-loss re-upload).
#[test]
fn image_blob_ships_to_main() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("image-basic").unwrap();

    // The bridge shipped exactly one `UploadImage` at `createImageResource`
    // time; main inserted it into its `ImageResourceMap`.
    let count = app.with_app(|a| a.backend().image_resource_count());
    assert_eq!(count, 1, "main should retain the shipped pixel Blob");

    // Settle a few more frames — the bridge ships once per decode (no
    // staging, no re-shipping), so the count stays at 1.
    for _ in 0..3 {
        app.wait_for_timeout(std::time::Duration::from_millis(16));
    }
    let count = app.with_app(|a| a.backend().image_resource_count());
    assert_eq!(count, 1, "images ship exactly once per decode");
}
