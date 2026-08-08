// Android stub: `tur-android` registers `NativeHttp` (reqwest + rustls on the
// shared tokio runtime), so `tur:net` is available — re-export it. No
// file-picker backend exists, so `tur:filepicker` is stubbed to an empty
// namespace. Swapped in for the Android build (resolve.alias in
// rspack.config.ts). Case code that imports `filePicker` resolves to
// `undefined` (no-op); `request` works normally.
import * as Net from "tur:net";

export { Net };
export const FilePicker: Record<string, unknown> = {};
