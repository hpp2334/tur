// tur engine Web Worker bootstrap.
//
// Imported as a module worker (`new Worker(blobURL, { type: "module" })`).
// Loads the SAME wasm bindgen shim + shared wasm module/memory the main
// thread uses, then hands control to Rust via `tur_worker_main(ptr)`.
//
// Unlike wasm_thread's bootstrap this does NOT call `close()` after the
// entry point — the worker stays alive because Rust installs a
// `self.onmessage` wake handler (in `tur_worker_main`) that keeps its JS
// event loop alive, and the cooperative mini-executor drives `loop_fut`
// via `setTimeout(0)` repolls. Calling `close()` would terminate the
// worker the moment the Rust entry returned, which would be immediately
// (the entry sets up the executor and returns; it does NOT block).
import init, { tur_worker_main } from "WASM_BINDGEN_SHIM_URL";

self.onmessage = (event) => {
    // The init message carries `[module, memory, ptr]` (an Array). Wake
    // messages are a bare number (`0`) — they can arrive before
    // `tur_worker_main` has installed its own `onmessage` wake handler
    // (main sends right after spawn). Ignore them here: the data they wake
    // for is already in the shared-memory mpsc, drained by the worker's
    // initial poll once the Rust wake handler is installed.
    if (!Array.isArray(event.data)) return;
    const [module, memory, ptr] = event.data;
    init(module, memory)
        .then(() => {
            tur_worker_main(ptr);
        })
        .catch((err) => {
            console.error("tur worker init failed:", err);
            // Propagate to the main `onerror` handler.
            setTimeout(() => {
                throw err;
            });
        });
};
