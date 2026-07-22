/**
 * @tur/clipboard — ambient type declarations for the clipboard module.
 *
 * The runtime is a synthetic boa module registered by `tur-clipboard`
 * (Rust crate) under the specifier `"tur:clipboard"`. It exports a
 * single `clipboard` object whose methods are Promise-returning fns backed
 * by the engine's async executor — `readText()` reads from the platform
 * clipboard, `writeText(text)` writes to it.
 *
 * The `Clipboard` trait impl is injected by the embedder (tur-wasm wires
 * `navigator.clipboard`; tests use `RecordingClipboard`). Not available in
 * headless engine-only contexts unless an embedder provides a backend.
 */

declare module "tur:clipboard" {
    export interface Clipboard {
        /** Read text from the clipboard. Resolves with the text (empty
         *  string if denied/unavailable). */
        readText(): Promise<string>;

        /** Write text to the clipboard. Resolves when the write has been
         *  acknowledged by the platform. */
        writeText(text: string): Promise<void>;
    }

    export const clipboard: Clipboard;
}
