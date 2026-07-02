import type { Color } from "./index";

// `__tur.colorLerp` is registered alongside `createColor` in the Rust bridge
// (`core/bridge/color.rs::tur_color_lerp`) and delegates to `Color::lerp` in
// tur-shared. We declare only the slice of `__tur` this module needs.
declare const __tur: {
    colorLerp(a: Color, b: Color, t: number): Color;
};

/**
 * A linear interpolation between a beginning and ending numeric value,
 * mirroring Flutter's `Tween<double>`.
 *
 * Used with an `AnimationController`'s `onTick` to drive a source atom:
 *
 * ```ts
 * const width$ = source(100);
 * const tween = Tween({ begin: 100, end: 280 });
 * const ctrl = createAnimationController({
 *     duration: 1000,
 *     onTick: mutate((_ctx, t) => set(width$, tween.lerp(t))),
 * });
 * ctrl.forward();
 * ```
 *
 * `begin` and `end` are mutable (getters + setters), matching Flutter's
 * Tween — changes are honoured the next time `lerp` runs.
 */
export interface TweenValue {
    /** The value at the start of the animation (`t = 0`). Mutable. */
    begin: number;
    /** The value at the end of the animation (`t = 1`). Mutable. */
    end: number;
    /** Interpolate at parameter `t`. `t` is NOT clamped (matches Flutter). */
    lerp(t: number): number;
    /** Interpolate at parameter `t`, clamped to `[0, 1]`. */
    transform(t: number): number;
}

export function Tween(opts: { begin: number; end: number }): TweenValue {
    let begin = opts.begin;
    let end = opts.end;
    return {
        get begin() {
            return begin;
        },
        set begin(v: number) {
            begin = v;
        },
        get end() {
            return end;
        },
        set end(v: number) {
            end = v;
        },
        lerp(t: number): number {
            return begin + (end - begin) * t;
        },
        transform(t: number): number {
            return begin + (end - begin) * Math.max(0, Math.min(1, t));
        },
    };
}

/**
 * A linear interpolation between two `Color` values, mirroring Flutter's
 * `ColorTween` (which delegates to `Color.lerp`). Channels are interpolated
 * component-wise in u8 space on the Rust side via `__tur.colorLerp`.
 *
 * `begin` and `end` are mutable, matching Flutter.
 */
export interface ColorTweenValue {
    begin: Color;
    end: Color;
    lerp(t: number): Color;
    transform(t: number): Color;
}

export function ColorTween(opts: {
    begin: Color;
    end: Color;
}): ColorTweenValue {
    let begin = opts.begin;
    let end = opts.end;
    return {
        get begin() {
            return begin;
        },
        set begin(v: Color) {
            begin = v;
        },
        get end() {
            return end;
        },
        set end(v: Color) {
            end = v;
        },
        lerp(t: number): Color {
            return __tur.colorLerp(begin, end, t);
        },
        transform(t: number): Color {
            return __tur.colorLerp(begin, end, Math.max(0, Math.min(1, t)));
        },
    };
}
