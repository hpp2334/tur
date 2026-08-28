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
// from the engine (the `now$` ticker — its async loop captures the
// mutation ctx). Everything else in the app is ctx-only: reactive reads
// happen in `derive` closures, side effects in `mutate` closures, and
// actions compose by dispatching other mutations via `ctx.set(action, …)`.

import type { Store } from "tur:core";
import { mount } from "tur:std";
import { setInstanceStore, startNowTicker, stopNowTicker } from "./state";
import { Shell } from "./views/shell";

export function start({ store }: { store: Store }) {
    // Stash the instance store for the case store's pass-through (embedded
    // seam cases bind their `__*` test hooks to it — see cases/*-invalidation).
    setInstanceStore(store);
    mount(Shell);
    store.set(startNowTicker);
    return stopNowTicker;
}
