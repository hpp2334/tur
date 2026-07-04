/**
 * @tur/animation-ext — Flutter-style animation toolkit built on the
 * `builtin:tur/core` primitives.
 *
 * Provides `Tween` / `ColorTween` (mutable begin/end interpolation channels)
 * and the `AnimatedContainer` / `AnimatedOpacity` / `AnimatedPositioned`
 * implicit-animation family (Flutter's `ImplicitlyAnimatedWidget`). Everything
 * is composed in TS from the core primitives `ReadableSubscribe` + `Tween` +
 * `createAnimationController` — no native element is involved.
 *
 * Each animatable prop becomes a "channel": a Tween/ColorTween seeded at the
 * prop's initial value, displayed as `tween.lerp(progress)`. One
 * `AnimationController` drives a shared `progress` source via `onTick`. When a
 * reactive target changes, `ReadableSubscribe.onUpdate$` rebases each
 * channel's `begin` to its currently-displayed value, sets `end` to the new
 * target, and restarts the controller. Static (non-readable) props never
 * retarget — they pass through.
 */

import {
    type ContainerProps,
    type Element,
    type Mutation,
    type Readable,
    type Val,
    type Color,
    Container,
    Positioned,
    Opacity,
    ReadableSubscribe,
    source,
    get,
    set,
    derive,
    mutate,
    createAnimationController,
    colorLerp,
} from "builtin:tur/core";

// ---------------------------------------------------------------------------
// Curve keyword (mirror of the engine-side curve enum string).
// ---------------------------------------------------------------------------

export type Curve = "linear" | "easeIn" | "easeOut" | "easeInOut";

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
        lerp(t: number) {
            return begin + (end - begin) * t;
        },
        transform(t: number) {
            return begin + (end - begin) * Math.max(0, Math.min(1, t));
        },
    };
}

export function ColorTween(opts: { begin: Color; end: Color }): ColorTweenValue {
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
        lerp(t: number) {
            return colorLerp(begin, end, t);
        },
        transform(t: number) {
            return colorLerp(begin, end, Math.max(0, Math.min(1, t)));
        },
    };
}

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

// Precisely detects reactive atom handles by probing `get`, which the bridge
// validates and rejects (throws) for non-atoms. Carries the current value so
// callers avoid a second read.
interface AtomProbe {
    atom: false;
}
interface AtomProbeHit<T> {
    atom: true;
    handle: Readable<T>;
    value: T;
}
function probeAtom<T>(v: Val<T>): AtomProbe | AtomProbeHit<T> {
    if (typeof v !== "object" || v === null) return { atom: false };
    try {
        const handle = v as Readable<T>;
        return { atom: true, handle, value: get(handle) };
    } catch {
        return { atom: false };
    }
}

// Resolve a `Val<T>` to its current static value (read once, then fixed).
function resolveStatic<T>(v: Val<T> | undefined, fallback: T): T {
    if (v == null) return fallback;
    const probe = probeAtom(v);
    return probe.atom ? probe.value : (v as T);
}

type Retarget = () => void;

// Register one animatable channel: returns a `derive(() => tween.lerp(progress))`
// for reactive props, or the static value unchanged for non-reactive props.
function animChannel<T>(
    target: Val<T>,
    progress: Readable<number>,
    makeTween: (initial: T) => TweenLike<T>,
    retargets: Retarget[],
    readables: Readable<unknown>[],
): Val<T> {
    const probe = probeAtom(target);
    if (!probe.atom) return target;
    readables.push(probe.handle);
    const tween = makeTween(probe.value);
    const handle = probe.handle;
    retargets.push(() => {
        tween.begin = tween.lerp(get(progress));
        tween.end = get(handle);
    });
    return derive(() => tween.lerp(get(progress)));
}

function runRetargets(retargets: Retarget[], ctrl: { forward(): void }): void {
    for (const r of retargets) r();
    ctrl.forward();
}

export function AnimatedContainer(props: AnimatedContainerProps): Element {
    const duration = resolveStatic(props.duration, 300);
    const curve = resolveStatic(props.curve, "linear");
    const progress$ = source(1.0);
    const retargets: Retarget[] = [];
    const readables: Readable<unknown>[] = [];
    const num = (i: number): TweenValue => Tween({ begin: i, end: i });
    // `color` props are typed `Val<Brush | null>` but only solid `Color`s
    // interpolate (gradients snap to the new target); narrow to `Val<Color>`.
    const col = (i: Color): ColorTweenValue => ColorTween({ begin: i, end: i });
    const ch = <T>(
        v: Val<T> | undefined,
        mk: (initial: T) => TweenLike<T>,
    ): Val<T> | undefined =>
        v != null ? animChannel(v, progress$, mk, retargets, readables) : undefined;

    const child = Container({
        width: ch(props.width, num),
        height: ch(props.height, num),
        padding: ch(props.padding, num),
        color: ch(props.color as Val<Color> | undefined, col),
        borderColor: ch(props.borderColor as Val<Color> | undefined, col),
        borderWidth: ch(props.borderWidth, num),
        borderRadius: ch(props.borderRadius, num),
        shadowColor: ch(props.shadowColor as Val<Color> | undefined, col),
        shadowBlur: ch(props.shadowBlur, num),
        alignment: props.alignment,
        borderPosition: props.borderPosition,
        shadowOffset: props.shadowOffset,
        queryKey: props.queryKey,
        children: props.children,
    });

    const ctrl = createAnimationController({
        duration,
        curve,
        onTick: mutate((_c, t) => set(progress$, t)),
        onEnd: props.onEnd,
    });

    return ReadableSubscribe({
        readables,
        onUpdate$: mutate(() => runRetargets(retargets, ctrl)),
        child,
    });
}

export function AnimatedOpacity(props: {
    value: Val<number>;
    duration: Val<number>;
    curve?: Val<Curve>;
    onEnd?: Mutation<[], void>;
    child?: Element;
    queryKey?: Val<string[]>;
}): Element {
    const duration = resolveStatic(props.duration, 300);
    const curve = resolveStatic(props.curve, "linear");
    const progress$ = source(1.0);
    const retargets: Retarget[] = [];
    const readables: Readable<unknown>[] = [];
    const num = (i: number): TweenValue => Tween({ begin: i, end: i });

    const value = animChannel(props.value, progress$, num, retargets, readables);
    const ctrl = createAnimationController({
        duration,
        curve,
        onTick: mutate((_c, t) => set(progress$, t)),
        onEnd: props.onEnd,
    });

    return ReadableSubscribe({
        readables,
        onUpdate$: mutate(() => runRetargets(retargets, ctrl)),
        child: Opacity({ value, child: props.child, queryKey: props.queryKey }),
    });
}

export function AnimatedPositioned(props: {
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
}): Element {
    const duration = resolveStatic(props.duration, 300);
    const curve = resolveStatic(props.curve, "linear");
    const progress$ = source(1.0);
    const retargets: Retarget[] = [];
    const readables: Readable<unknown>[] = [];
    const num = (i: number): TweenValue => Tween({ begin: i, end: i });
    const ch = (v: Val<number> | undefined): Val<number> | undefined =>
        v != null ? animChannel(v, progress$, num, retargets, readables) : undefined;

    const child = Positioned({
        left: ch(props.left),
        top: ch(props.top),
        right: ch(props.right),
        bottom: ch(props.bottom),
        width: ch(props.width),
        height: ch(props.height),
        child: props.child,
    });

    const ctrl = createAnimationController({
        duration,
        curve,
        onTick: mutate((_c, t) => set(progress$, t)),
        onEnd: props.onEnd,
    });

    return ReadableSubscribe({
        readables,
        onUpdate$: mutate(() => runRetargets(retargets, ctrl)),
        child,
    });
}
