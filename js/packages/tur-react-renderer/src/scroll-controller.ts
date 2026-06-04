import type {
    ScrollController,
    ScrollControllerOptions,
} from "./tur";

export function createScrollController(
    options?: ScrollControllerOptions,
): ScrollController {
    return __tur.createScrollController(__tur.__ctx, options);
}
