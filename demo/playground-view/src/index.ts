// tur playground-view entry point. Bundled as a single ES module (`impl.js`)
// by rspack and loaded by the website's wasm host via
// `TurWebsiteApp.loadAndRunModule`. `setViewRoot(viewRoot("main"), Shell)`
// mounts the UI.

import { setViewRoot, viewRoot } from "tur:std";
import { Shell } from "./views/shell";

setViewRoot(viewRoot("main"), Shell);
