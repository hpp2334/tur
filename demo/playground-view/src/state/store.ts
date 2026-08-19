import { createStore } from "tur:std";

/**
 * The playground's single root store — created once here and mounted by the
 * entry point (`mount(store, Shell)` in `src/index.ts`).
 *
 * Every state module (`state/*`) and view (`views/*`) imports THIS store so
 * module-level `store.get` / `store.set` hit the same atoms the mounted tree
 * reads (a second `createStore()` would hold independent values for the same
 * declarations — the store IS the KV).
 */
export const store = createStore();
