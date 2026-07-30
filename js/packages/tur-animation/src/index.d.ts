/**
 * @tur-ng/animation — ambient type declarations for the animation library.
 *
 * Runtime is a synthetic boa module registered by `tur-animation` under the
 * specifier `"tur:animation"`. It is a single combined module that:
 *
 * 1. Re-exports the native bridge fn `createAnimationController` from the
 *    engine-internal `tur:animation/native` module.
 * 2. Defines the JS-only implicit-animation widgets `AnimatedContainer`,
 *    `AnimatedOpacity`, `AnimatedPositioned` plus the `Tween` / `ColorTween`
 *    interpolation channels.
 *
 * The widgets are composed entirely from `tur:std` primitives
 * (`ReadableSubscribe` + `Tween` + `createAnimationController`) — the only
 * native elements involved (`Opacity`/`Transform`/`Container`/`Positioned`)
 * all ship as part of `tur:std`.
 */

declare module "tur:animation" {
    import type {
        Color,
        ContainerProps,
        Element,
        Mutation,
        Val,
    } from "tur:std";

    // ---------------------------------------------------------------------------
    // Curve keyword (mirror of the engine-side curve enum string).
    // ---------------------------------------------------------------------------

    export type Curve = "linear" | "easeIn" | "easeOut" | "easeInOut";

    // ---------------------------------------------------------------------------
    // Animation controller (native).
    // ---------------------------------------------------------------------------

    export type AnimationStatus =
        | "stopped"
        | "forward"
        | "reverse"
        | "completed"
        | "paused";

    export interface AnimationControllerOpts {
        duration?: number;
        curve?: Curve;
        repeat?: number | "infinite";
        onTick?: Mutation<[number], void>;
        onEnd?: Mutation<[], void>;
    }

    export interface AnimationController {
        readonly value: number;
        readonly status: AnimationStatus;
        readonly duration: number;
        readonly speed: number;
        forward(): void;
        reverse(): void;
        stop(): void;
        pause(): void;
        resume(): void;
        seek(t: number): void;
        setSpeed(factor: number): void;
        repeat(count: number | "infinite"): void;
    }

    export function createAnimationController(
        opts?: AnimationControllerOpts,
    ): AnimationController;

    // ---------------------------------------------------------------------------
    // Tween / ColorTween — Flutter-style begin/end interpolation with mutable
    // endpoints. Pair with an `AnimationController`'s `onTick` to drive a source.
    // ---------------------------------------------------------------------------

    /** A mutable begin/end interpolation channel over values of type `T`. */
    export interface TweenLike<T> {
        /** Value at the start of the animation (`t = 0`). Mutable. */
        begin: T;
        /** Value at the end of the animation (`t = 1`). Mutable. */
        end: T;
        /** Interpolate at parameter `t`. `t` is NOT clamped (matches Flutter). */
        lerp(t: number): T;
        /** Interpolate at parameter `t`, clamped to `[0, 1]`. */
        transform(t: number): T;
    }

    export interface TweenValue extends TweenLike<number> {}
    export interface ColorTweenValue extends TweenLike<Color> {}

    export function Tween(opts: { begin: number; end: number }): TweenValue;
    export function ColorTween(opts: {
        begin: Color;
        end: Color;
    }): ColorTweenValue;

    // ---------------------------------------------------------------------------
    // AnimatedContainer / AnimatedOpacity / AnimatedPositioned
    // ---------------------------------------------------------------------------

    export interface AnimatedContainerProps extends ContainerProps {
        /** Animation duration in milliseconds. Required. */
        duration: Val<number>;
        /** Easing curve keyword (default `"linear"`). */
        curve?: Val<Curve>;
        /** Fired once when an in-flight implicit animation completes. */
        onEnd?: Mutation<[], void>;
    }

    export function AnimatedContainer(props: AnimatedContainerProps): Element;

    export interface AnimatedOpacityProps {
        value: Val<number>;
        duration: Val<number>;
        curve?: Val<Curve>;
        onEnd?: Mutation<[], void>;
        child?: Element;
        queryKey?: Val<string[]>;
    }

    export function AnimatedOpacity(props: AnimatedOpacityProps): Element;

    export interface AnimatedPositionedProps {
        left?: Val<number>;
        top?: Val<number>;
        right?: Val<number>;
        bottom?: Val<number>;
        width?: Val<number>;
        height?: Val<number>;
        duration: Val<number>;
        curve?: Val<Curve>;
        onEnd?: Mutation<[], void>;
        child: Element;
        queryKey?: Val<string[]>;
    }

    export function AnimatedPositioned(props: AnimatedPositionedProps): Element;
}
