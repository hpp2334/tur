import type { Store } from "tur:core";

// The engine-provided instance store, stashed once by the entry module's
// `start({ store })`. Nothing in the app reads it directly for reactive
// access (all reactive access is ctx-only) — it exists solely as the
// pass-through argument when the case store invokes a compiled case's own
// `start({ store })` (seam cases bind their `__*` test hooks to it).
let instanceStore: Store | null = null;

export function setInstanceStore(store: Store): void {
    instanceStore = store;
}

export function getInstanceStore(): Store | null {
    return instanceStore;
}
