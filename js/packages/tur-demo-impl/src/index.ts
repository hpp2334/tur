// tur playground entry point. Bundled as a single IIFE (`impl.js`) by rspack
// and loaded by tur-demo's wasm host via `TurWasmApp.load_and_run_js`.
//
// Import order matters:
//   1. `./bootstrap` sets `globalThis.TurEdgy` — MUST run before any module
//      that triggers case compilation (case-store.ts cache priming).
//   2. `./views/shell` transitively loads `./state` (which primes the
//      case cache as a side effect) and the view tree.
//   3. `render(Shell)` mounts the UI.

import "./bootstrap";
import { render } from "@tur/edgy";
import { Shell } from "./views/shell";

render(Shell);
