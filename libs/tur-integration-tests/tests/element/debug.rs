use std::cell::Ref;

use tur_engine::core::element::{NodeId, ElementNodeId};
use tur_engine::core::elements::NodeTreeData;
use tur_integration_tests::TurTestApp;

fn print_tree(tree: &Ref<NodeTreeData>) {
    fn go(tree: &NodeTreeData, id: NodeId, depth: usize) {
        if let Some(node) = tree.get_element(ElementNodeId::new(id.as_u64())) {
            eprintln!(
                "{}{} id={} children={}",
                "  ".repeat(depth),
                node.element.as_ref().unwrap().kind(),
                node.id,
                node.children.len(),
            );
            for child in &node.children {
                go(tree, *child, depth + 1);
            }
        }
    }

    if let Some(root) = tree.root_element() {
        go(tree, root.id.into(), 0);
    }
}

#[test]
fn debug_raw_column_basic() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-basic").unwrap();

    eprintln!("=== raw JS: column-basic ===");
    print_tree(&app.element_tree());
}

#[test]
fn debug_solidjs_column_basic() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-basic").unwrap();

    eprintln!("=== SolidJS: column-basic ===");
    print_tree(&app.element_tree());
}

#[test]
fn debug_react_minimal() {
    let app = TurTestApp::new(400.0, 600.0).unwrap();
    let source = std::fs::read_to_string(std::path::Path::new(
        &std::env::var("CARGO_MANIFEST_DIR").unwrap()
    ).parent().unwrap().parent().unwrap()
     .join("js/packages/tur-test-cases/dist/column-basic.js"))
     .unwrap();
    let _ = app.eval_module_source(&source);
    let tree = app.element_tree();
    let root_id = tree.root_element_id();
    let child_count = tree.raw_children_of_element(root_id.unwrap()).len();
    assert!(child_count > 0, "root should have children, got {child_count}");
}
