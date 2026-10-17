//! Dev-tool snapshots — the engine-native `turDevTool` data plane.
//!
//! The JS-rail bridge (an in-engine `turDevTool` global evaluated through
//! `eval_js`) is gone; the same snapshots are produced here as **JSON
//! strings** straight off the engine's Rust state, surfaced to embedders
//! through [`TurApp`](crate::TurApp) RPCs (`dev_tool_element_tree` /
//! `dev_tool_get_element` / `dev_tool_frame_stats`). The wasm host exposes
//! them as the page-level `turDevTool` global's methods — same shapes as
//! the old JS objects, so page-side consumers (agent-browser evals, the
//! perf bench) parse unchanged JSON.
//!
//! Serialization is a tiny hand-rolled writer (no serde dep): every value
//! is a string / number / bool / null / array / object with known shapes.

use crate::core::app::frame_stats::{FrameStats, FrameTiming, HostFrameTiming};
use crate::core::elements::{DevNodeData, NodeTreeData, TraceValue};

// ---------------------------------------------------------------------------
// JSON writing
// ---------------------------------------------------------------------------

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn num(n: f64) -> String {
    if n.is_finite() {
        format!("{n}")
    } else {
        "null".to_string()
    }
}

fn trace_value_json(v: &TraceValue) -> String {
    match v {
        TraceValue::Str(s) => json_escape(s),
        TraceValue::Num(n) => num(*n),
        TraceValue::Bool(b) => b.to_string(),
        TraceValue::Null => "null".to_string(),
    }
}

fn kv_object(entries: &[(&str, String)]) -> String {
    let inner: Vec<String> = entries
        .iter()
        .map(|(k, v)| format!("{}:{v}", json_escape(k)))
        .collect();
    format!("{{{}}}", inner.join(","))
}

// ---------------------------------------------------------------------------
// Node snapshots
// ---------------------------------------------------------------------------

/// JSON for one node — the `turDevTool` element shape:
/// `{ id, name, label, props, layout:{relative,absolute,width,height,extra?}, queryKey?, children:[{id}, ...] }`.
pub fn dev_node_json(node: &DevNodeData) -> String {
    let props: Vec<(&str, String)> = node
        .props
        .iter()
        .map(|(k, v)| (*k, trace_value_json(v)))
        .collect();
    let mut layout = vec![
        (
            "relative",
            kv_object(&[("x", num(node.relative.0)), ("y", num(node.relative.1))]),
        ),
        (
            "absolute",
            kv_object(&[("x", num(node.absolute.0)), ("y", num(node.absolute.1))]),
        ),
        ("width", num(node.size.0)),
        ("height", num(node.size.1)),
    ];
    if !node.layout_extra.is_empty() {
        let extra: Vec<(&str, String)> = node
            .layout_extra
            .iter()
            .map(|(k, v)| (*k, trace_value_json(v)))
            .collect();
        layout.push(("extra", kv_object(&extra)));
    }

    let mut fields = vec![
        ("id", format!("{}", node.id.as_u64())),
        ("name", json_escape(node.name)),
        ("label", json_escape(&node.label)),
        ("props", kv_object(&props)),
        ("layout", kv_object(&layout)),
    ];
    if let Some(keys) = &node.query_key {
        let arr: Vec<String> = keys.iter().map(|k| json_escape(k)).collect();
        fields.push(("queryKey", format!("[{}]", arr.join(","))));
    }
    let children: Vec<String> = node
        .children
        .iter()
        .map(|id| kv_object(&[("id", format!("{}", id.as_u64()))]))
        .collect();
    fields.push(("children", format!("[{}]", children.join(","))));
    kv_object(&fields)
}

/// JSON snapshot of the root node, or `"null"` if no tree is mounted.
pub fn element_tree_json(tree: &NodeTreeData) -> String {
    let Some(root_id) = tree.root_element_id() else {
        return "null".to_string();
    };
    match tree.dev_tool_node(root_id.into()) {
        Some(node) => dev_node_json(&node),
        None => "null".to_string(),
    }
}

/// JSON snapshot of a single node by id. Returns `"null"` if not found.
pub fn get_element_json(tree: &NodeTreeData, id: crate::core::element::NodeId) -> String {
    match tree.dev_tool_node(id) {
        Some(node) => dev_node_json(&node),
        None => "null".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Frame stats
// ---------------------------------------------------------------------------

fn frame_timing_json(t: &FrameTiming) -> String {
    kv_object(&[
        ("frame", format!("{}", t.frame_id)),
        ("nodesWalked", num(t.nodes_walked as f64)),
        ("opsRecorded", num(t.ops_recorded as f64)),
        ("commands", num(t.commands_emitted as f64)),
        ("batchBytes", num(t.batch_bytes as f64)),
        ("dirtyLayoutNodes", num(t.dirty_layout_nodes as f64)),
        ("flushUs", num(t.flush_us as f64)),
        ("layoutUs", num(t.layout_us as f64)),
        ("recordWalkUs", num(t.record_walk_us as f64)),
        ("batchPostUs", num(t.batch_post_us as f64)),
    ])
}

fn host_timing_json(t: &HostFrameTiming) -> String {
    kv_object(&[
        ("frame", format!("{}", t.frame_id)),
        ("applyUs", num(t.apply_us as f64)),
        ("presentUs", num(t.present_us as f64)),
    ])
}

/// JSON frame-stats snapshot:
/// `{ flushes, paintedFrames, totals:{...}, last, lastHost, hostTimingEnabled }`.
/// `last` is the most recent painted frame's worker-side timing (or `null`);
/// `lastHost` the most recent host render-commit timing (`applyUs` /
/// `presentUs`) — populated only while frame timing is enabled.
pub fn frame_stats_json(stats: &FrameStats) -> String {
    let totals = kv_object(&[
        ("flushUs", num(stats.total_flush_us.get() as f64)),
        ("nodesWalked", num(stats.total_nodes_walked.get() as f64)),
        ("opsRecorded", num(stats.total_ops_recorded.get() as f64)),
    ]);
    let last = stats
        .last
        .borrow()
        .as_ref()
        .map(frame_timing_json)
        .unwrap_or_else(|| "null".to_string());
    let last_host = stats
        .last_host
        .borrow()
        .as_ref()
        .map(host_timing_json)
        .unwrap_or_else(|| "null".to_string());
    kv_object(&[
        ("flushes", format!("{}", stats.flushes.get())),
        ("paintedFrames", format!("{}", stats.painted_frames.get())),
        ("totals", totals),
        ("last", last),
        ("lastHost", last_host),
        (
            "hostTimingEnabled",
            stats.host_timing_enabled.get().to_string(),
        ),
    ])
}
