//! The lazy-font rail end-to-end (the rut kit surface): declare →
//! `launch_future(load_font_data(..))` → the epoch-keyed memo re-shape →
//! the swap.
//!
//! The fixture renders a `Text` under a custom-named face ("Tur Custom")
//! whose bytes the harness's canned provider serves (Roboto Mono under the
//! declared name — the rename pin, through the REAL rows). Before the load
//! lands, the label shapes on the default family (wider advances); after
//! it, monospace — asserted through the caret-x cell width the editor pins
//! use, here via the element's layout width delta.

use std::time::Duration;

use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

const ZERO: Duration = Duration::ZERO;

const LAZY_FONT_RUT: &str = r#"
use futures::{ launch_future };
use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::text::core::{ Text };
use tur_font_kit::{ declare_font, load_font_data };
use tur_kit::reactive::{ sv };

entry fn start() -> opaque {
    declare_font("Tur Custom");
    // Text renders on the default family NOW (the declared-but-unloaded
    // face shapes nothing); the load launches via the `load` probe below
    // so the test controls the swap timing deterministically.
    mount(Column().child(
        Text().text(sv("AIl1.")).font_family("Tur Custom").font_size(13.0).query_key("label").build()
    ).build());
    return opaque(nil);
}

// The test's swap trigger — launch (NOT await) for the fallback-then-swap
// UX the module author gets. (The cx param rides the control-entry channel;
// unused.)
entry fn load(_cx: opaque) {
    launch_future(load_font_data("Tur Custom"));
}
"#;

/// Natural width of the query-keyed label element.
fn label_width(app: &TurTestApp) -> f64 {
    let id = app
        .query_element(&["label"])
        .expect("the lazy-font label");
    let b = app
        .get_element_absolute_bounds(ElementNodeId::new(id.as_u64()))
        .unwrap();
    b.right - b.left
}

#[test]
fn lazy_font_declares_swaps_after_load() {
    let mut app = TurTestApp::new(400.0, 200.0).unwrap();
    app.load_rut_module(LAZY_FONT_RUT).unwrap();
    app.wait_for_timeout(ZERO);
    let cx = app.rut_start_answer();

    // Pre-load: the declared-but-unloaded family shapes on the default
    // family (proportional — "AIl1." renders narrow relative to mono).
    let before = label_width(&app);

    // Trigger the load, then poll until the epoch-keyed memos re-shape
    // (the fetch/registration is async — bounded polling, the canned
    // provider resolves immediately once a pump drives the task).
    app.call_rut_entry_cx("load", cx).unwrap();
    let mut after = before;
    for _ in 0..200 {
        app.wait_for_timeout(Duration::from_millis(10));
        after = label_width(&app);
        if (after - before).abs() > 0.5 {
            break;
        }
    }

    // Roboto Mono is the fixture's canned face: monospace advances make
    // "AIl1." WIDER than the default sans's proportional layout (the
    // narrow 'I'/'l' expand to the full cell). The swap must move the width.
    assert!(
        (after - before).abs() > 0.5,
        "the label must re-shape after the lazy load: before={before} after={after}"
    );
    assert!(
        after > before,
        "the mono swap widens 'AIl1.': before={before} after={after}"
    );
}
