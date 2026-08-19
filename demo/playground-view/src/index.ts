// tur playground-view entry point. Bundled as a single ES module (`impl.js`)
// by rspack and loaded by the website's wasm host via
// `TurWebsiteApp.loadAndRunModule`. The module lifecycle contract requires
// a `start()` export: the engine invokes it after eval (and runs the
// returned cleanup before the next load / at destroy). The root-tree
// lifecycle itself is engine-owned (mount replaces / teardown clears).
//
// No module store is kept anywhere: `start()` creates the store, mounts the
// Shell with it, and dispatches the one boot mutation that needs a writer
// from the engine (the `now$` ticker — its launch loop captures the
// mutation ctx). Everything else in the app is ctx-only: reactive reads
// happen in `derive` closures, side effects in `mutate` closures, and
// actions compose by dispatching other mutations via `ctx.set(action, …)`.

import { createStore, mount } from "tur:std";
import { startNowTicker, stopNowTicker } from "./state";
import { Shell } from "./views/shell";

export function start() {
    const store = createStore();
    mount(store, Shell);
    store.set(startNowTicker);
    return stopNowTicker;
}
