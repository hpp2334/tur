//! Integration tests for the `tur:filepicker` bridge.
//!
//! Validates the full spawn → tick → complete → drain → PromiseJob →
//! reactive-set path end-to-end, mirroring `async_bridge.rs`:
//!
//! 1. JS calls `filePicker.pick(opts)` / `filePicker.saveFile(name, bytes)`
//!    (ctx-bound methods on the `filePicker` const object).
//! 2. The fn creates a pending `JsPromise`, spawns a future via the engine's
//!    `AsyncExecutor` that calls `FilePickerBackend::pick(opts).await`
//!    (`RecordingFilePicker` resolves eagerly).
//! 3. `flush`'s `tick` polls the future, the future pushes a `Completion`
//!    that resolves the promise (building the JS `{name,bytes: ArrayBuffer,
//!    type, size}` objects there).
//! 4. `drain_completions` runs the completion under `&mut Context`, enqueuing
//!    a `PromiseJob`.
//! 5. boa's `executor.drain` runs the PromiseJob → fires the `.then` body.
//!
//! Capability lookup: the bridge fns read their `Rc<dyn FilePickerBackend>`
//! from `TurJsContext`'s capability registry (populated by `TurFilePickerPlugin`
//! during `register`).

use tur_filepicker_capability::PickedFile;
use tur_integration_tests::TurTestApp;

#[test]
fn pick_resolves_with_files_and_drives_reactive_set() {
    let mut app = TurTestApp::new_with_filepicker(200.0, 100.0).unwrap();

    // Pre-canned picked files.
    app.set_next_pick(vec![
        PickedFile {
            name: "a.txt".to_string(),
            bytes: b"hello".to_vec(),
            mime_type: Some("text/plain".to_string()),
        },
        PickedFile {
            name: "b.bin".to_string(),
            bytes: vec![0, 1, 2, 3],
            mime_type: None,
        },
    ]);

    app.eval_module_source(
        r#"
        import { source, set } from "tur:std";
        import { filePicker } from "tur:filepicker";
        globalThis.__count$ = source(0);
        globalThis.__first$ = source("");
        globalThis.__size$ = source(0);
        filePicker.pick({ multiple: true }).then((files) => {
            set(globalThis.__count$, files.length);
            set(globalThis.__first$, files[0].name);
            set(globalThis.__size$, files[0].size);
            globalThis.__result_count = String(files.length);
            globalThis.__result_name = files[0].name;
            globalThis.__result_size = String(files[0].size);
            globalThis.__result_type = files[0].type;
        });
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__result_count") == "2");

    assert_eq!(app.eval_js("globalThis.__result_count"), "2");
    assert_eq!(app.eval_js("globalThis.__result_name"), "a.txt");
    assert_eq!(app.eval_js("globalThis.__result_size"), "5");
    assert_eq!(app.eval_js("globalThis.__result_type"), "text/plain");
}

#[test]
fn pick_returns_empty_array_when_cancelled() {
    let mut app = TurTestApp::new_with_filepicker(200.0, 100.0).unwrap();
    // No canned pick → backend resolves with an empty Vec (cancelled).

    app.eval_module_source(
        r#"
        import { source, set } from "tur:std";
        import { filePicker } from "tur:filepicker";
        globalThis.__len$ = source(-1);
        filePicker.pick().then((files) => {
            set(globalThis.__len$, files.length);
            globalThis.__result_len = String(files.length);
        });
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__result_len") == "0");
    assert_eq!(app.eval_js("globalThis.__result_len"), "0");
}

#[test]
fn save_file_logs_to_recording() {
    let mut app = TurTestApp::new_with_filepicker(200.0, 100.0).unwrap();

    // `saveFile` spawns the save on the worker's executor; the spawned
    // future is polled asynchronously after the load-module RPC returns
    // (and after each frame's `FrameOutcome` is shipped). `settle()` can
    // exit after a single frame — before the worker has polled the save
    // future — so we synchronize on the promise's `.then` (which fires only
    // after `FilePickerBackend::save_file` has logged the save), mirroring
    // the `pick` tests. This eliminates the main↔worker race that flaked
    // under CI.
    app.eval_module_source(
        r#"
        import { filePicker } from "tur:filepicker";
        // Build a 4-byte ArrayBuffer and save it.
        const bytes = new ArrayBuffer(4);
        const view = new Uint8Array(bytes);
        view[0] = 10; view[1] = 20; view[2] = 30; view[3] = 40;
        filePicker.saveFile("out.bin", bytes).then(() => {
            globalThis.__saved = "1";
        });
        "#,
    )
    .unwrap();
    app.wait_for(|a| a.eval_js("globalThis.__saved") == "1");

    let save = app.last_save().expect("expected one saveFile call");
    assert_eq!(save.name, "out.bin");
    assert_eq!(save.bytes, vec![10, 20, 30, 40]);
}
