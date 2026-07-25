// tur playground-view entry point. Bundled as a single ES module (`impl.js`)
// by rspack and loaded by the website's wasm host via
// `TurWebsiteApp.loadAndRunModule`. `render(Shell)` mounts the UI.

import { render } from "tur:std";
import { Shell } from "./views/shell";

render(Shell);
