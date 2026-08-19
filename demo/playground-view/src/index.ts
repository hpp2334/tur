// tur playground-view entry point. Bundled as a single ES module (`impl.js`)
// by rspack and loaded by the website's wasm host via
// `TurWebsiteApp.loadAndRunModule`. The module lifecycle contract requires
// a `start()` export: the engine invokes it after eval (and runs the
// returned cleanup before the next load / at destroy). The root-tree
// lifecycle itself is engine-owned (mount replaces / teardown clears), so
// no cleanup is needed here.

import { mount } from "tur:std";
import { store } from "./state/store";
import { Shell } from "./views/shell";

export function start() {
    mount(store, Shell);
}
