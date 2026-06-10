import type { AnimationController, AnimationControllerOptions } from "./tur";

export function createAnimationController(
    options?: AnimationControllerOptions,
): AnimationController {
    return __tur.createAnimationController(__tur.__ctx, options);
}
