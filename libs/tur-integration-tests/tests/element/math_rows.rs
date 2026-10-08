//! The `math_sin` / `math_cos` rows — pure f64 → f64 trig on the `tur_host`
//! pkg (radians in, radians out; no instance state, no reactivity).
//!
//! Pinned headlessly through `start`'s u64 answer (the standard probe
//! channel): the module packs rounded thousandths of sin(0.5) and
//! cos(0.5) into one integer, so the EXACT values are pinned — a row
//! that swapped sin/cos, degrees-for-radians, or dropped precision
//! fails the answer.

use tur_integration_tests::TurTestApp;

const MATH_RUT: &str = r#"
use tur_host::{ math_cos, math_sin, mount };
use tur_kit::{ Text };

entry fn start() -> u64 {
    let sin_mils = (math_sin(0.5) * 1000.0 + 0.5) as u64;
    let cos_mils = (math_cos(0.5) * 1000.0 + 0.5) as u64;
    mount(Text().text("math").query_key("math/root").build());
    return sin_mils * 10000 + cos_mils;
}
"#;

#[test]
fn math_sin_cos_rows_answer_exact_values() {
    let app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(MATH_RUT).unwrap();
    // sin(0.5) ≈ 0.479426 → 479 mils; cos(0.5) ≈ 0.877583 → 878 mils.
    assert_eq!(app.rut_start_answer(), 479 * 10000 + 878);
}
