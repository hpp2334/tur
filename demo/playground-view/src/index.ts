// tur playground-view entry point. Bundled as a single ES module (`impl.js`)
// by rspack and loaded by the website's wasm host via
// `TurWebsiteApp.loadAndRunModule`. The module lifecycle contract requires a
// `start` export: the engine invokes it after eval with the INSTANCE store
// (`start({ store })` — a live `{get, set}` object the instance-owned tree is
// born-bound to) and runs the returned cleanup before the next load / at
// destroy. The root-tree lifecycle itself is engine-owned (mount replaces /
// teardown clears).
//
// No store is created here: the module mounts the Shell with the injected
// instance store and dispatches the one boot mutation that needs a writer
// from the engine (the `now$` ticker — its launch loop captures the
// mutation ctx). Everything else in the app is ctx-only: reactive reads
// happen in `derive` closures, side effects in `mutate` closures, and
// actions compose by dispatching other mutations via `ctx.set(action, …)`.

import type { Store } from "tur:core";
import { mount } from "tur:std";
import { startNowTicker, stopNowTicker } from "./state";
import { Shell } from "./views/shell";

export function start({ store }: { store: Store }) {
    // Stash the instance store for embedded case helpers (module-scope test
    // seams reach it via `globalThis.__store` — see cases/*-invalidation).
    globalThis.__store = store;
    mount(Shell);
    store.set(startNowTicker);
    return stopNowTicker;
}
