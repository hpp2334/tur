use tur_integration_tests::TurTestApp;

const RUNTIME: &str = r#"
const T = globalThis.__tur;
const ctx = T.__ctx;
// Source atom holding an EdgyElement (spec handle).
globalThis.__elem = T.source(
    ctx,
    T.Text(ctx, { text: "first", queryKey: ["dyn_first"] }),
);
const root = T.Dynamic(ctx, { child: globalThis.__elem });
T.render(ctx, root);
"#;

#[test]
fn dynamic_mounts_initial_element() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_bundle_source(RUNTIME).unwrap();
    app.render();

    assert!(
        app.query_element(&["dyn_first"]).is_some(),
        "initial element should be mounted",
    );
}

#[test]
fn dynamic_rebuilds_when_atom_changes() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_bundle_source(RUNTIME).unwrap();
    app.render();
    assert!(app.query_element(&["dyn_first"]).is_some());

    // Replace the element held by the atom with a brand-new spec object.
    app.eval_js(
        r#"globalThis.__tur.set(
            globalThis.__tur.__ctx,
            globalThis.__elem,
            globalThis.__tur.Text(
                globalThis.__tur.__ctx,
                { text: "second", queryKey: ["dyn_second"] },
            ),
        );"#,
    );
    app.render();

    assert!(
        app.query_element(&["dyn_first"]).is_none(),
        "old element should be torn down",
    );
    assert!(
        app.query_element(&["dyn_second"]).is_some(),
        "new element should be mounted",
    );
}

#[test]
fn dynamic_no_rebuild_when_atom_re_set_to_same_object() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_bundle_source(RUNTIME).unwrap();
    app.render();

    // Capture the currently-mounted element object identity (via a global ref).
    app.eval_js(
        r#"globalThis.__captured = globalThis.__tur.get(globalThis.__tur.__ctx, globalThis.__elem);"#,
    );
    let first_id = app.query_element(&["dyn_first"]).unwrap();

    // Re-set the SAME object — Dynamic must treat it as unchanged (ptr_eq).
    app.eval_js(
        r#"globalThis.__tur.set(globalThis.__tur.__ctx, globalThis.__elem, globalThis.__captured);"#,
    );
    app.render();

    let first_id_after = app.query_element(&["dyn_first"]).unwrap();
    assert_eq!(
        first_id, first_id_after,
        "same object must not trigger a rebuild",
    );
}
