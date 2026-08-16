// tur playground-view entry point. Bundled as a single ES module (`impl.js`)
// by rspack and loaded by the website's wasm host via
// `TurWebsiteApp.loadAndRunModule`. `mount(Shell)` builds the view tree.

import { mount } from "tur:std";
import { Shell } from "./views/shell";

mount(Shell);
