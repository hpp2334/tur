import type { Store } from "@tur-ng/core";

declare global {
    // Test-seam: module-scope helpers reach the instance store via
    // `globalThis.__store` (set by the playground entry's `start({ store })`).
    var __store: Store;
}

export {};
