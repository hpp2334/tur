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

/// <reference types="@tur-ng/std" />
/// <reference types="@tur-ng/core" />

declare module "tur:animation" {
    import type {
        Alignment,
        BorderPosition,
        Color,
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
    // AnimatedContainer / AnimatedOpacity / AnimatedPositioned — builder-
    // pattern JS widgets mirroring the `tur:std` builders: chainable prop
    // methods returning `this`, terminated by `.build()`.
    // ---------------------------------------------------------------------------

    export interface AnimatedContainerBuilder {
        width(v: Val<number>): this;
        height(v: Val<number>): this;
        padding(v: Val<number>): this;
        color(v: Val<Color | null>): this;
        borderColor(v: Val<Color | null>): this;
        borderWidth(v: Val<number>): this;
        borderRadius(v: Val<number>): this;
        shadowColor(v: Val<Color | null>): this;
        shadowBlur(v: Val<number>): this;
        alignment(v: Val<Alignment>): this;
        borderPosition(v: Val<BorderPosition>): this;
        shadowOffset(v: [number, number]): this;
        queryKey(keys: Val<string[]>): this;
        children(children: Element[]): this;
        /** Animation duration in milliseconds (default `300`). */
        duration(v: Val<number>): this;
        /** Easing curve keyword (default `"linear"`). */
        curve(v: Val<Curve>): this;
        /** Fired once when an in-flight implicit animation completes. */
        onEnd(m: Mutation<[], void>): this;
        build(): Element;
    }

    export function AnimatedContainer(): AnimatedContainerBuilder;

    export interface AnimatedOpacityBuilder {
        value(v: Val<number>): this;
        duration(v: Val<number>): this;
        curve(v: Val<Curve>): this;
        onEnd(m: Mutation<[], void>): this;
        child(child: Element): this;
        queryKey(keys: Val<string[]>): this;
        build(): Element;
    }

    export function AnimatedOpacity(): AnimatedOpacityBuilder;

    export interface AnimatedPositionedBuilder {
        left(v: Val<number>): this;
        top(v: Val<number>): this;
        right(v: Val<number>): this;
        bottom(v: Val<number>): this;
        width(v: Val<number>): this;
        height(v: Val<number>): this;
        duration(v: Val<number>): this;
        curve(v: Val<Curve>): this;
        onEnd(m: Mutation<[], void>): this;
        child(child: Element): this;
        build(): Element;
    }

    export function AnimatedPositioned(): AnimatedPositionedBuilder;
}
