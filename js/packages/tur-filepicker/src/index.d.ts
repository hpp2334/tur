/**
 * @tur-ng/filepicker — ambient type declarations for the file-picker module.
 *
 * The runtime is a synthetic boa module registered by `tur-filepicker-capability`
 * (`TurFilePickerPlugin`) under the specifier `"tur:filepicker"`. It exports a
 * single `filePicker` object whose methods are Promise-returning fns backed by
 * the engine's async executor — `pick()` opens the platform file picker,
 * `saveFile()` persists bytes via the platform save dialog / download.
 *
 * The `FilePickerBackend` impl is injected by the embedder (tur-wasm wires the
 * `<input type=file>` / `<a download>` browser backend; tests use
 * `RecordingFilePicker`). File picking is **opt-in**: there is no no-op
 * default, so a host that wants `tur:filepicker` must register a real backend
 * via `.capability(FilePicker::new(backend))` + `.plugin(TurFilePickerPlugin)`.
 * Code that imports `tur:filepicker` without the plugin installed fails fast
 * (the engine builder rejects a `requires` it can't satisfy; or, if the plugin
 * itself is absent, the module loader rejects the `tur:filepicker` specifier).
 */

declare module "tur:filepicker" {
    /** Options for {@link FilePicker.pick}. */
    export interface PickOptions {
        /** Accepted file filters — MIME types (`"image/*"`) or extensions
         *  (`".png"`). Platform-dependent how each is honored (browsers accept
         *  both via `<input accept>`; native `rfd` derives extensions only). */
        accept?: string[];
        /** Allow selecting more than one file. `pick` always resolves with an
         *  array; `multiple: false` yields at most one entry. */
        multiple?: boolean;
    }

    /** Options for {@link FilePicker.saveFile}. */
    export interface SaveOptions {
        /** Suggested save filters (MIME/extension). Platform-dependent. */
        accept?: string[];
    }

    /** A picked file: name + raw bytes + best-effort MIME type + size. */
    export interface PickedFile {
        /** File name (no path). */
        name: string;
        /** Raw file bytes. */
        bytes: ArrayBuffer;
        /** MIME type when the platform reports one (e.g. `"image/png"`); empty
         *  string when unknown. */
        type: string;
        /** File size in bytes. */
        size: number;
    }

    /** File-picker surface. */
    export interface FilePicker {
        /** Open the platform file picker. Resolves with the picked files
         *  (empty array if cancelled/denied). */
        pick(opts?: PickOptions): Promise<PickedFile[]>;
        /** Persist `bytes` under file name `name` (via the platform save dialog
         *  or a browser download). Resolves when written. */
        saveFile(
            name: string,
            bytes: ArrayBuffer,
            opts?: SaveOptions,
        ): Promise<void>;
    }

    export const filePicker: FilePicker;
}
