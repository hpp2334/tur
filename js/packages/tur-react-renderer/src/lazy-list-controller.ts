import type { LazyListController, LazyListControllerOptions } from "./tur";

export function createLazyListController(
    options?: LazyListControllerOptions,
): LazyListController {
    return __tur.createLazyListController(__tur.__ctx, options);
}
