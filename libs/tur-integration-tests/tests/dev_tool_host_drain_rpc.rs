//! Pins the **host-drain** RPC reply transport (the wasm browser path) on
//! native. `new_with_host_drain_rpc` wraps the harness spawner so the
//! engine sees an executor that cannot re-poll host tasks from a worker
//! waker — exactly the `wasm_bindgen_futures` semantics that made
//! `turDevTool.elementTree()` hang in browsers. Under that transport the
//! dev-tool RPCs must still resolve: the reply rides
//! `HostMsg::DevToolReply`, the looper drains it on the host thread, and
//! `apply_msg` fires the oneshot there (see
//! `WorkerExecutor::wakes_host_tasks_cross_thread`).

use tur_integration_tests::TurTestApp;

const TREE_RUT: &str = r#"
use tur::{ mount };
use tur_kit::{ Column, Text };

entry fn start() {
    let mut col = Column.new();
    col.child(Text.new().text("host-drain rpc").query_key("probe").build());
    mount(col.build());
}
"#;

#[test]
fn dev_tool_rpcs_resolve_over_host_drain_transport() {
    let app = TurTestApp::new_with_host_drain_rpc(200.0, 100.0).expect("app builds");
    app.load_rut_module(TREE_RUT).expect("module loads");
    let _ = app.pump();

    // elementTree: the browser surface's exact RPC (String transport).
    // Children are bare `{id}` handles — fetch the child node for content.
    let tree = app.dev_tool_element_tree_json();
    assert!(
        tree.contains("tur_root"),
        "elementTree JSON must carry the root, got: {tree}"
    );
    let mut node = tree;
    for _ in 0..4 {
        let child = node
            .split("\"children\":[{\"id\":")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .and_then(|id| id.trim().parse::<u64>().ok());
        let Some(id) = child else { break };
        node = app.dev_tool_get_element_json(id);
        if node.contains("host-drain rpc") {
            break;
        }
    }
    let leaf = node;
    assert!(
        leaf.contains("host-drain rpc"),
        "walking to the leaf must reach the mounted text; last node: {leaf}"
    );
    assert!(
        leaf.contains("probe"),
        "the text node must carry the query key, got: {leaf}"
    );

    // frameStats: same transport, different payload.
    let stats = app.dev_tool_frame_stats();
    assert!(
        stats.contains("flushes"),
        "frameStats JSON must carry the flush counter, got: {stats}"
    );
}
