/**
 * @tur-ng/clipboard — ambient type declarations for the clipboard module.
 *
 * The runtime is a synthetic boa module registered by `tur-clipboard`
 * (Rust crate) under the specifier `"tur:clipboard"`. It exports a
 * single `clipboard` object whose methods are Task-returning fns backed
 * by the engine's async executor — `readText()` reads from the platform
 * clipboard, `writeText(text)` writes to it.
 *
 * Every method returns `Task<T> = { promise, cancel() }` (see `tur:std`):
 * await `task.promise`; `task.cancel()` aborts the wait (a dispatched host
 * op may still complete underneath — its result is discarded) and rejects
 * the promise with a `CancelError`.
 *
 * The `Clipboard` trait impl is injected by the embedder (tur-wasm wires
 * `navigator.clipboard`; tests use `RecordingClipboard`). Not available in
 * headless engine-only contexts unless an embedder provides a backend.
 */

/// <reference types="@tur-ng/std" />

declare module "tur:clipboard" {
    import type { Task } from "tur:std";

    export interface Clipboard {
        /** Read text from the clipboard. `promise` resolves with the text
         *  (empty string if denied/unavailable). */
        readText(): Task<string>;

        /** Write text to the clipboard. `promise` resolves when the write
         *  has been acknowledged by the platform. */
        writeText(text: string): Task<void>;
    }

    export const clipboard: Clipboard;
}
