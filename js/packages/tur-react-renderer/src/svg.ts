import type { ResourceHandle } from "./tur";

export function createSvgResource(svgString: string): ResourceHandle {
    return __tur.createSvgResource(__tur.__ctx, svgString);
}
