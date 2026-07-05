// tur playground entry point. Bundled as a single ES module (`impl.js`) by
// rspack and loaded by tur-demo's wasm host via
// `TurWasmApp.load_and_run_module`. `render(Shell)` mounts the UI.

import { render } from "builtin:tur/std";
import { Shell } from "./views/shell";

render(Shell);
