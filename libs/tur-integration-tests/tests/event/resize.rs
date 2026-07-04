use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// A Column with a single `Expanded` child that fills the viewport's main
/// axis. Its height must track the viewport size across resizes — which only
/// happens if the resize cascade re-lays-out the whole subtree (the
/// `mark_root_dirty` fix), not just the root.
const RESIZE_BUNDLE: &str = r#"
import { render, Column, Expanded, Container } from "builtin:tur/core";
render(Column({
    children: [
        Expanded({ child: Container({ queryKey: ["fill"] }) }),
    ],
}));
"#;

fn fill_height(app: &TurTestApp) -> f64 {
    let id = app.query_element(&["fill"]).unwrap();
    let id = ElementNodeId::new(id.as_u64());
    let b = app.get_element_absolute_bounds(id).unwrap();
    b.bottom - b.top
}

#[test]
fn resize_reflows_descendants() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.eval_module_source(RESIZE_BUNDLE).unwrap();
    app.render();

    let h0 = fill_height(&app);
    assert!(
        (h0 - 300.0).abs() < 1.0,
        "fill child should be 300px tall initially, got {h0}",
    );

    // Grow the viewport; the Expanded descendant must reflow to the new height.
    app.resize(400.0, 600.0);
    app.render();
    let h1 = fill_height(&app);
    assert!(
        (h1 - 600.0).abs() < 1.0,
        "fill child should reflow to 600px after resize, got {h1}",
    );

    // Shrink back.
    app.resize(400.0, 200.0);
    app.render();
    let h2 = fill_height(&app);
    assert!(
        (h2 - 200.0).abs() < 1.0,
        "fill child should reflow to 200px after shrink, got {h2}",
    );
}
