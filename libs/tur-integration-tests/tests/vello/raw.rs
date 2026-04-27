use super::vello_app::TurVelloApp;

pub fn vello_counter_app() {
    let app = TurVelloApp::new(1024.0, 768.0, 1.0).unwrap();
    app.load_bundle("vello-column-basic").unwrap();

    let tree = app.element_tree();
    let tree = tree.borrow();
    let root = tree.root().unwrap();
    assert!(root.children.len() > 0);
    drop(tree);

    app.render();
}
