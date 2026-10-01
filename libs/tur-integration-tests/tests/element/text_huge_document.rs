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

use tur::{ el_build, el_input_ctrl, el_qkey, el_scroll, mount, tctrl_new, tctrl_push_span };

entry fn start() {
    let ctrl = tctrl_new();
    let mut i = 0;
    while (i < 4000) {
        tctrl_push_span(ctrl, f"const value{i} = {i}; // line {i}\n");
        i += 1;
    }

    let input = el_input_ctrl(ctrl, 100000.0, 4000.0, 14.0);
    el_qkey(input, "ed");
    let scroller = el_scroll(true, el_build(input));
    el_qkey(scroller, "scroll");
    mount(el_build(scroller));
}
"##;

#[test]
fn huge_document_layout_and_paint_does_not_panic() {
    let mut app = TurTestApp::new(400.0, 600.0).expect("app");
    app.load_rut_module(HUGE_EDITOR)
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
