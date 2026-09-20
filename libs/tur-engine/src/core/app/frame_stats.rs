//! Per-instance frame statistics — the render-performance probe.
//!
//! Worker-side counters are always-on and cheap (a handful of `Cell` bumps
//! per painted frame); they surface to JS via `turDevTool.frameStats()`.
//! Host-side timings (the render commit point's apply + present) are
//! **opt-in** — `turDevTool.enableHostFrameTiming()` toggles the host's
//! collector over the event bus on the reserved channel, and the host ships
//! one JSON payload per painted frame back on the same channel
//! ([`FRAME_TIMING_CHANNEL`]).
//!
//! Stats never gate or drive behavior — this is measurement only.

use std::cell::{Cell, RefCell};

/// Reserved event-bus channel for host frame-timing payloads
/// (host→JS JSON) and the enable/disable toggle (JS→host, `"on"`/`"off"`).
pub const FRAME_TIMING_CHANNEL: u64 = 1;

/// Timing + counters for one painted frame.
#[derive(Debug, Default, Clone)]
pub struct FrameTiming {
    /// The flush epoch this frame belongs to.
    pub frame_id: u64,
    // ── Worker-side counters ──
    /// Element nodes entered during the record walk (post-cull — culled
    /// subtrees are never entered).
    pub nodes_walked: u64,
    /// Canvas ops recorded into the batch (all nodes' segments summed).
    pub ops_recorded: u64,
    /// Paint commands emitted (nodes with own draws; interleaving parents
    /// emit one command per segment).
    pub commands_emitted: u64,
    /// Approximate wire size of the batch (command/ops count × item size).
    pub batch_bytes: u64,
    /// Nodes that actually performed layout this flush (the dirty +
    /// constraints-changed short-circuit did not hit).
    pub dirty_layout_nodes: u64,
    // ── Worker-side timings (µs, rounded) ──
    /// Total `flush()` duration.
    pub flush_us: u64,
    /// Layout step duration (summed over fixed-point iterations).
    pub layout_us: u64,
    /// Record walk (tree.paint into RecordingCanvas).
    pub record_walk_us: u64,
    /// Batch post-processing (`into_render_commands`).
    pub batch_post_us: u64,
}

/// The pieces `build_render_batch` reports (walk + post timings + counters).
#[derive(Debug, Default, Clone)]
pub struct FrameTimingParts {
    pub walk_us: u64,
    pub post_us: u64,
    pub nodes_walked: u64,
    pub ops_recorded: u64,
    pub commands_emitted: u64,
    pub batch_bytes: u64,
}

/// Estimated wire size of one frame's batch: count × item size. The actual
/// serialization cost (postMessage structured-clone on wasm) scales with
/// the same counts, so this is the honest proxy.
pub fn estimate_batch_bytes(commands: usize, ops: usize) -> u64 {
    let command_size = std::mem::size_of::<crate::core::render::RenderCommand>();
    let op_size = std::mem::size_of::<crate::core::render::CanvasOp>();
    (commands * command_size + ops * op_size) as u64
}

/// Host-side render-commit timings for one painted frame, pushed back via
/// `WorkerMsg::FrameTiming` when frame timing is enabled. Kept in a
/// **separate slot** from the worker-side `FrameTiming` — frames pipeline
/// (the worker may record N+1 before frame N's host timings arrive), so
/// merging would misattribute; consumers align by `frame_id` if needed.
#[derive(Debug, Default, Clone)]
pub struct HostFrameTiming {
    /// The flush epoch of the frame these timings belong to.
    pub frame_id: u64,
    /// Scene rebuild + command playback (`Renderer::render_commands`).
    pub apply_us: u64,
    /// Encode + raster + composite (`Renderer::present`).
    pub present_us: u64,
}

/// Cumulative worker-side frame statistics — one per instance, lives on
/// [`TurInstanceContext`](crate::core::js_runtime::TurInstanceContext)
/// (`frame_stats`), shared across every cheap clone.
#[derive(Debug, Default)]
pub struct FrameStats {
    /// `flush()` calls so far (all frames, painted or not).
    pub flushes: Cell<u64>,
    /// Flushes that produced a render batch.
    pub painted_frames: Cell<u64>,
    /// Cumulative totals (worker side) — denominators for averages.
    pub total_flush_us: Cell<u64>,
    pub total_nodes_walked: Cell<u64>,
    pub total_ops_recorded: Cell<u64>,
    /// Timing/counters of the most recent painted frame.
    pub last: RefCell<Option<FrameTiming>>,
    /// Host-side render-commit timings of the most recent frame the host
    /// applied. Separate slot from `last` — see [`HostFrameTiming`] (frames
    /// pipeline, so merging would misattribute).
    pub last_host: RefCell<Option<HostFrameTiming>>,
    /// Host-side frame-timing collection toggle (`turDevTool`
    /// `.setHostFrameTiming(true/false)`; mirrored by `HostBackend`).
    pub host_timing_enabled: Cell<bool>,
}

impl FrameStats {
    /// Record host-side render-commit timings for one painted frame.
    pub fn record_host_timing(&self, timing: HostFrameTiming) {
        *self.last_host.borrow_mut() = Some(timing);
    }

    /// Record one painted frame's timing, replacing `last` and bumping the
    /// cumulative counters.
    pub fn record_painted(&self, timing: FrameTiming) {
        self.flushes.set(self.flushes.get() + 1);
        self.painted_frames.set(self.painted_frames.get() + 1);
        self.total_flush_us
            .set(self.total_flush_us.get() + timing.flush_us);
        self.total_nodes_walked
            .set(self.total_nodes_walked.get() + timing.nodes_walked);
        self.total_ops_recorded
            .set(self.total_ops_recorded.get() + timing.ops_recorded);
        *self.last.borrow_mut() = Some(timing);
    }

    /// Record an unpainted flush (counters only — timing slots untouched).
    pub fn record_idle_flush(&self) {
        self.flushes.set(self.flushes.get() + 1);
    }
}

impl FrameTiming {
    /// JSON encoding for logging / JS consumption.
    pub fn encode_json(&self) -> String {
        format!(
            "{{\"frame\":{},\"nodesWalked\":{},\"opsRecorded\":{},\"commands\":{},\"batchBytes\":{},\
             \"dirtyLayoutNodes\":{},\"flushUs\":{},\"layoutUs\":{},\"recordWalkUs\":{},\
             \"batchPostUs\":{}}}",
            self.frame_id,
            self.nodes_walked,
            self.ops_recorded,
            self.commands_emitted,
            self.batch_bytes,
            self.dirty_layout_nodes,
            self.flush_us,
            self.layout_us,
            self.record_walk_us,
            self.batch_post_us,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The JSON payload is parseable and carries every field (the JS side
    /// `JSON.parse`s it — the shape contract).
    #[test]
    fn encode_json_carries_all_fields() {
        let timing = FrameTiming {
            frame_id: 7,
            nodes_walked: 42,
            ops_recorded: 100,
            commands_emitted: 39,
            batch_bytes: 4800,
            dirty_layout_nodes: 3,
            flush_us: 1200,
            layout_us: 500,
            record_walk_us: 300,
            batch_post_us: 100,
        };
        let json = timing.encode_json();
        for field in [
            "\"frame\":7",
            "\"nodesWalked\":42",
            "\"opsRecorded\":100",
            "\"commands\":39",
            "\"batchBytes\":4800",
            "\"dirtyLayoutNodes\":3",
            "\"flushUs\":1200",
            "\"layoutUs\":500",
            "\"recordWalkUs\":300",
            "\"batchPostUs\":100",
        ] {
            assert!(json.contains(field), "missing {field} in {json}");
        }
    }

    /// Cumulative counters accumulate; `last` holds the most recent.
    #[test]
    fn frame_stats_cumulate() {
        let stats = FrameStats::default();
        stats.record_painted(FrameTiming {
            frame_id: 1,
            flush_us: 100,
            nodes_walked: 10,
            ops_recorded: 5,
            ..FrameTiming::default()
        });
        stats.record_idle_flush();
        stats.record_painted(FrameTiming {
            frame_id: 3,
            flush_us: 200,
            nodes_walked: 20,
            ops_recorded: 15,
            ..FrameTiming::default()
        });
        assert_eq!(stats.flushes.get(), 3);
        assert_eq!(stats.painted_frames.get(), 2);
        assert_eq!(stats.total_flush_us.get(), 300);
        assert_eq!(stats.total_nodes_walked.get(), 30);
        let last = stats.last.borrow().clone().unwrap();
        assert_eq!(last.frame_id, 3);
        assert_eq!(last.ops_recorded, 15);
    }
}
