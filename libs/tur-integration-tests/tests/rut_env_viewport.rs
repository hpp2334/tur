//! The engine-environment viewport atoms, end to end: `TurStdPlugin` mints
//! two f64 backing sources (`viewport_width` / `viewport_height`) at
//! register time, the `env` rows answer their atom ids, and the kit's
//! `viewport_width()` / `viewport_height()` wrappers hand them to rut as
//! `Readable<f64>` (CSS-pixel logical units).
//!
//! Pins:
//! - the mobile-breakpoint predicate — `derive<bool>(viewport_width() <
//!   720)` — drives a Condition (the mobile branch at the 390×800 boot
//!   viewport),
//! - a Text bound over a one-level derive of the same predicate renders
//!   the mobile label, and a shell resize to 1280×800 republishes the
//!   atoms so the derive re-evaluates (the desktop label + the Condition
//!   flip),
//! - the raw atom reads answer the live sizes (the ctx-held ids stay
//!   stable across calls — one mint per instance).

use std::time::Duration;

use tur_integration_tests::TurTestApp;

/// The bound label's text, found through its query key.
fn text_at(app: &TurTestApp, key: &'static str) -> String {
    let id = app.query_element(&["rut", key]).expect("text not found");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<tur_engine::builtin_plugins::text::TextElement>()
            .map(|c| {
                c.spans()
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
            })
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

const ENV_VIEWPORT_RUT: &str = r#"
use tur_kit::control_flow::control::{ Condition };
use tur_kit::handles::{ View, mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ DeriveCtx, Readable, derive, sv, viewport_height, viewport_width };
use tur_kit::text::core::{ Text };

// The context the embedder holds between entry calls: the two env atom
// ids (taken at the mint).
struct EnvCx {
    w: u64,
    h: u64,
}

entry fn start() -> opaque {
    let w: Readable<f64> = viewport_width();
    // The mobile breakpoint — the predicate exactly as the playground's
    // mobile view will spell it: a derive over the env atom, dep-tracked
    // through the ctx read.
    let mobile: Readable<bool> = derive<bool>(fn (ctx: DeriveCtx) -> bool {
        return ctx.get<f64>(w) < 720.0;
    });
    // The bound-Text consumption: a derive reading the env atom directly
    // (one level — a derive reading a DERIVED atom would need a nested VM
    // face call, which the no-reentrancy law refuses).
    let label: Readable<str> = derive<str>(fn (ctx: DeriveCtx) -> str {
        if (ctx.get<f64>(w) < 720.0) {
            return "mobile";
        }
        return "desktop";
    });
    let col = Column()
        .child(Text().text(label).query_key("rut/label").build())
        .child(
        Condition(mobile)
            .then_build(fn () -> View {
                return Text().text(sv("cond: mobile")).query_key("rut/cond").build();
            })
            .else_build(fn () -> View {
                return Text().text(sv("cond: desktop")).query_key("rut/cond").build();
            })
            .build(),
    );
    mount(col.build());
    return opaque(EnvCx { w: w.atom_id(), h: viewport_height().atom_id() });
}

// The raw-atom probe: the ids held since the mint are still the env rows'
// atoms, and their reads serve the live sizes ("WxH").
entry fn probe_raw(cx: opaque) -> str {
    let c = opaque::downcast<EnvCx>(cx);
    if (c == nil) {
        panic("env fixture: cx is not an EnvCx");
    }
    if (c.w != viewport_width().atom_id()) {
        panic("env fixture: the width atom id drifted");
    }
    if (c.h != viewport_height().atom_id()) {
        panic("env fixture: the height atom id drifted");
    }
    return f"{viewport_width().get() as u64}x{viewport_height().get() as u64}";
}
"#;

#[test]
fn viewport_env_atoms_drive_a_bound_label_across_a_resize() {
    let mut app = TurTestApp::new(390.0, 800.0).unwrap();
    app.load_rut_module(ENV_VIEWPORT_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    assert_eq!(
        text_at(&app, "label"),
        "mobile",
        "390 < 720 — the derived label renders the mobile side"
    );
    assert_eq!(
        text_at(&app, "cond"),
        "cond: mobile",
        "the derive<bool> drives the Condition's mobile branch"
    );
    let cx = app.rut_start_answer();
    assert_eq!(
        app.call_rut_entry_cx_str("probe_raw", cx).unwrap(),
        "390x800",
        "the raw env atoms read the boot viewport"
    );

    // The resize republishes both atoms; the derives re-run in the flush
    // fixed-point and the bound label + Condition branch re-render.
    app.resize(1280.0, 800.0);
    assert!(
        app.wait_for(|a| text_at(a, "label") == "desktop"),
        "1280 >= 720 — the derive re-evaluated to the desktop label"
    );
    assert_eq!(
        text_at(&app, "cond"),
        "cond: desktop",
        "the Condition flipped to the desktop branch"
    );
    assert_eq!(
        app.call_rut_entry_cx_str("probe_raw", cx).unwrap(),
        "1280x800",
        "the raw env atoms read the resized viewport"
    );
}
