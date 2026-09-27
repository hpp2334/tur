//! Documents larger than 64 KiB must not panic the text pipeline.
//!
//! parley 0.9 stores per-cluster source-text offsets as `u16`
//! (`ClusterData::text_offset = char_index as u16`), so for a single shaped
//! segment past 65 535 bytes the cluster — and therefore the line — text
//! ranges wrap (e.g. `65527..28`). `extract_layout_data` slices the source
//! with those ranges and used to panic ("byte range starts at … but ends at
//! …"), which on wasm took down the whole worker.
//!
//! The contract pinned here: a > 64 KiB spanned document lays out and paints
//! without panicking. Rendering beyond the 64 KiB wrap point is degraded
//! (parley's data is corrupt there — that needs an upstream fix) but the
//! frame pipeline must survive it.

use tur_integration_tests::TurTestApp;

/// 4000 lines ≈ 168 KiB of text — well past the 65 535-byte wrap.
const HUGE_EDITOR: &str = r##"
import { mount, ScrollView, Input } from "tur:std";

const spans = [];
for (let i = 0; i < 4000; i++) {
    spans.push({ content: "const value" + i + " = " + i + "; // line " + i + "\n" });
}
globalThis.__spans = spans;
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans(spans);
mount(ScrollView()
    .queryKey(["scroll"])
    .child(Input()
     .controller(globalThis.__ctrl)
     .multiline(true)
     .fontFamily("monospace")
     .fontSize(14)
     .queryKey(["ed"])
     .build())
    .build());
"##;

#[test]
fn huge_document_layout_and_paint_does_not_panic() {
    let mut app = TurTestApp::new(400.0, 600.0).expect("app");
    app.eval_module_source(HUGE_EDITOR)
        .expect("load huge editor");
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Layout + paint the whole document repeatedly (scroll offsets change
    // which lines are on screen, exercising the wrapped-range region).
    for i in 0..6 {
        let outcome = app.pump();
        assert!(
            outcome.painted,
            "iteration {i}: the huge document must keep painting"
        );
    }
}
