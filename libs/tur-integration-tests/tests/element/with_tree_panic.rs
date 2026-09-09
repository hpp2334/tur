use std::panic::AssertUnwindSafe;

use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// Regression (harness contract): an assertion failing inside a
/// `with_element`/`with_tree` closure must FAIL the test. The closure runs
/// on the worker thread; before the fix, its panic skipped the reply send,
/// `with_tree` returned `None`, and any test that ignored the `Option`
/// passed vacuously — silently weakening every such assertion in the suite.
#[test]
fn with_element_panic_propagates_to_test_thread() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.eval_module_source(
        r#"
        import { mount, Container } from "tur:std";
        mount(Container().width(50).height(50).queryKey(["c"]).build());
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let id = ElementNodeId::new(app.query_element(&["c"]).unwrap().as_u64());

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        app.with_element(id, |_e| {
            panic!("deliberate failure inside the worker closure");
        });
    }));
    assert!(
        result.is_err(),
        "a panic inside the with_element closure must propagate to the \
         awaiting test, not vanish into a `None` reply"
    );

    // The worker survives the contained panic: the dispatch arm's RefCell
    // guards dropped normally and the loop kept running, so a follow-up
    // introspection (and the app itself) still works.
    let tree = app.element_tree();
    let node = tree.get_element(id).unwrap();
    assert_eq!(node.computed_layout.size.width, 50.0);
}

/// Same contract for a plain `assert!` failure (the common shape inside
/// test closures): the assertion message must reach the test thread.
#[test]
fn with_element_assert_failure_propagates() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.eval_module_source(
        r#"
        import { mount, Container } from "tur:std";
        mount(Container().width(50).height(50).queryKey(["c"]).build());
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let id = ElementNodeId::new(app.query_element(&["c"]).unwrap().as_u64());

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        app.with_element(id, |_e| {
            assert!(1 == 2, "sentinel-assert-message");
        });
    }));
    let err = result.expect_err("assert failure must propagate");
    let message = panic_message(&err);
    assert!(
        message.contains("sentinel-assert-message"),
        "the original payload message should surface, got: {message}"
    );
}

/// Downcast a caught panic payload to its display message (mirrors the
/// engine-side `panic_payload_message` helper).
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else {
        String::from("<non-string panic payload>")
    }
}
