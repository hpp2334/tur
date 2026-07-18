/**
 * @tur/animation — ambient type declarations for the animation library.
 *
 * Runtime is a synthetic boa module registered by `tur-animation` under the
 * specifier `"builtin:tur/animation"`. It is a single combined module that:
 *
 * 1. Re-exports the native bridge fns `Opacity`, `Transform`, and
 *    `createAnimationController` from the engine-internal `tur:animation/native`
 *    module.
 * 2. Defines the JS-only implicit-animation widgets `AnimatedContainer`,
 *    `AnimatedOpacity`, `AnimatedPositioned` plus the `Tween` / `ColorTween`
 *    interpolation channels.
 *
 * The widgets are composed entirely from `builtin:tur/std` primitives
 * (`ReadableSubscribe` + `Tween` + `createAnimationController`) — no native
 * element beyond `Opacity`/`Transform`/`Container`/`Positioned` is involved.
 */

declare module "builtin:tur/animation" {
    import type {
        Alignment,
        Color,
        ContainerProps,
        Element,
        Mutation,
        Val,
    } from "builtin:tur/std";

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
    // Opacity / Transform effect elements (native).
    // ---------------------------------------------------------------------------

    export interface OpacityProps {
        value: Val<number>;
        child?: Element;
        queryKey?: Val<string[]>;
    }

    export interface TransformProps {
        scale?: Val<number>;
        scaleX?: Val<number>;
        scaleY?: Val<number>;
        rotate?: Val<number>;
        translateX?: Val<number>;
        translateY?: Val<number>;
        /** Pivot for `rotate`/`scale`, within the child box. Defaults to
         *  `Alignment.Center` (matches Flutter's `Transform`). */
        alignment?: Val<Alignment>;
        child?: Element;
        queryKey?: Val<string[]>;
    }

    export function Opacity(props: OpacityProps): Element;
    export function Transform(props: TransformProps): Element;

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
