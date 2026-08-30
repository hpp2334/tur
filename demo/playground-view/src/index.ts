// tur playground-view entry point. Bundled as a single ES module (`impl.js`)
// by rspack and loaded by the website's wasm host via
// `TurWebsiteApp.loadAndRunModule`. The module lifecycle contract requires a
// `start` export: the engine invokes it after eval with the INSTANCE store
// (`start({ store })` — a live `{get, set}` object) and runs the returned
// cleanup before the next load / at destroy. The root-tree lifecycle itself
// is engine-owned (mount replaces / teardown clears).
//
// No store is created here: the module mounts the Shell with the injected
// instance store and dispatches the boot mutations that need a writer (the
// `now$` ticker — its async loop captures the mutation ctx — and the initial
// case spawn). Everything else in the app is ctx-only: reactive reads happen
// in `derive` closures, side effects in `mutate` closures, and actions
// compose by dispatching other mutations via `ctx.set(action, …)`.

import type { Store } from "tur:core";
import { mount } from "tur:std";
import { INITIAL_CASE, runCase, startNowTicker, stopNowTicker } from "./state";
import { Shell } from "./views/shell";

export function start({ store }: { store: Store }) {
    mount(Shell);
    store.set(startNowTicker);
    // Spawn the initial case's hosted child instance (compiles are primed
    // at module eval; this just binds the first controller).
    store.set(runCase, INITIAL_CASE);
    return stopNowTicker;
}
