import type { AnimationController, AnimationControllerOptions, TurNodeHandle } from "./tur";

export function createAnimationController(
    options?: AnimationControllerOptions,
): AnimationController {
    return __tur.createAnimationController(__tur.__ctx, options);
}

export function setNodeAttribute(
    handle: TurNodeHandle,
    key: string,
    value: unknown,
): void {
    __tur.setAttribute(__tur.__ctx, handle, key, value);
}
