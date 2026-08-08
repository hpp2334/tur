// Android stub: `tur-android` ships no `Http` backend (so `TurNetPlugin` skips
// `tur:net`) and no file-picker backend (so `tur:filepicker` is absent). Swapped
// in for the Android build (resolve.alias in rspack.config.ts) so the playground
// bundle has no `tur:net` / `tur:filepicker` imports at all. Case code that
// imports them resolves to these empty namespaces — `request`, `filePicker`,
// etc. are absent (no-op). Only the github-viewer case uses them; the ~80 other
// cases render unaffected.
export const FilePicker: Record<string, unknown> = {};
export const Net: Record<string, unknown> = {};
