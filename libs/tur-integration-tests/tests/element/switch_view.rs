use tur_integration_tests::TurTestApp;

const RUNTIME: &str = r#"
import { source, Switch, Text, setViewRoot, viewRoot } from "tur:std";
globalThis.__key = source("a");
const root = Switch({
    value: globalThis.__key,
    cases: [
        { key: "a", child: () => Text({ text: "AAA", queryKey: ["case_a"] }) },
        { key: "b", child: () => Text({ text: "BBB", queryKey: ["case_b"] }) },
    ],
    fallback: () => Text({ text: "FALL", queryKey: ["case_fallback"] }),
});
setViewRoot(viewRoot("main"), root);
"#;

#[test]
fn switch_mounts_initial_branch() {
    let app = TurTestApp::new(200.0, 100.0).unwrap();
    app.eval_module_source(RUNTIME).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // The "a" branch should be mounted; "b" and fallback should not.
    assert!(
        app.query_element(&["case_a"]).is_some(),
        "case_a should be mounted"
    );
    assert!(
        app.query_element(&["case_b"]).is_none(),
        "case_b should NOT be mounted"
    );
    assert!(
        app.query_element(&["case_fallback"]).is_none(),
        "fallback should NOT be mounted",
    );
}

#[test]
fn switch_swaps_branch_on_value_change() {
    let app = TurTestApp::new(200.0, 100.0).unwrap();
    app.eval_module_source(RUNTIME).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(app.query_element(&["case_a"]).is_some());

    // Flip the value atom to "b".
    app.eval_module_source(r#"import { set } from "tur:std"; set(globalThis.__key, "b");"#)
        .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(
        app.query_element(&["case_a"]).is_none(),
        "case_a should be torn down"
    );
    assert!(
        app.query_element(&["case_b"]).is_some(),
        "case_b should now be mounted",
    );
}

#[test]
fn switch_uses_fallback_when_no_case_matches() {
    let app = TurTestApp::new(200.0, 100.0).unwrap();
    app.eval_module_source(RUNTIME).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Value with no matching case → fallback branch.
    app.eval_module_source(r#"import { set } from "tur:std"; set(globalThis.__key, "zzz");"#)
        .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(app.query_element(&["case_a"]).is_none());
    assert!(app.query_element(&["case_b"]).is_none());
    assert!(
        app.query_element(&["case_fallback"]).is_some(),
        "fallback should be mounted when no case matches",
    );
}

#[test]
fn switch_no_rebuild_when_value_re_emits_same_key() {
    let app = TurTestApp::new(200.0, 100.0).unwrap();
    app.eval_module_source(RUNTIME).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let a_id = app.query_element(&["case_a"]).unwrap();
    // Re-set the same key — the mounted node identity should be unchanged.
    app.eval_module_source(r#"import { set } from "tur:std"; set(globalThis.__key, "a");"#)
        .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let a_id_after = app.query_element(&["case_a"]).unwrap();
    assert_eq!(a_id, a_id_after, "same key must not trigger a rebuild");
}

const DERIVED_RUNTIME: &str = r#"
import { source, derive, get, Switch, Text, setViewRoot, viewRoot } from "tur:std";
globalThis.__key = source("a");
globalThis.__derived = derive(() => get(globalThis.__key));
const root = Switch({
    value: globalThis.__derived,
    cases: [
        { key: "a", child: () => Text({ text: "AAA", queryKey: ["d_case_a"] }) },
        { key: "b", child: () => Text({ text: "BBB", queryKey: ["d_case_b"] }) },
    ],
    fallback: () => Text({ text: "FALL", queryKey: ["d_case_fallback"] }),
});
setViewRoot(viewRoot("main"), root);
"#;

#[test]
fn switch_swaps_branch_on_derived_value_change() {
    let app = TurTestApp::new(200.0, 100.0).unwrap();
    app.eval_module_source(DERIVED_RUNTIME).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(
        app.query_element(&["d_case_a"]).is_some(),
        "d_case_a should be mounted initially"
    );

    // Flip the source atom — the derived should go stale and the Switch
    // should swap via the subscriber graph (not a full-scan try_rebuild).
    app.eval_module_source(r#"import { set } from "tur:std"; set(globalThis.__key, "b");"#)
        .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(
        app.query_element(&["d_case_a"]).is_none(),
        "d_case_a should be torn down"
    );
    assert!(
        app.query_element(&["d_case_b"]).is_some(),
        "d_case_b should now be mounted"
    );
}
