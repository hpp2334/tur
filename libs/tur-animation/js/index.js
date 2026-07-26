/**
 * @tur-ng/animation — Flutter-style animation toolkit.
 *
 * This module is the consumer-facing surface of `tur-animation`. It exposes
 * both the native bridge fns (`Opacity`, `Transform`, `createAnimationController`,
 * re-exported from the internal `tur:animation/native` module) and the
 * JS-defined implicit-animation widgets (`AnimatedContainer`,
 * `AnimatedOpacity`, `AnimatedPositioned`, `Tween`, `ColorTween`).
 *
 * The widgets are composed entirely from `tur:std` primitives
 * (`ReadableSubscribe` + `Tween` + `createAnimationController`) — no native
 * element beyond `Opacity`/`Transform`/`Container`/`Positioned` is involved.
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
    Opacity,
    Transform,
    createAnimationController,
} from "tur:animation/native";

import {
    Container,
    Positioned,
    ReadableSubscribe,
    colorLerp,
    derive,
    get,
    mutate,
    set,
    source,
} from "tur:std";

// Re-export the native bridge fns so consumers can import everything from
// `tur:animation`.
export { Opacity, Transform, createAnimationController };

// ---------------------------------------------------------------------------
// Tween / ColorTween — Flutter-style begin/end interpolation with mutable
// endpoints. Pair with an `AnimationController`'s `onTick` to drive a source.
// ---------------------------------------------------------------------------

export function Tween(opts) {
    let begin = opts.begin;
    let end = opts.end;
    return {
        get begin() {
            return begin;
        },
        set begin(v) {
            begin = v;
        },
        get end() {
            return end;
        },
        set end(v) {
            end = v;
        },
        lerp(t) {
            return begin + (end - begin) * t;
        },
        transform(t) {
            return begin + (end - begin) * Math.max(0, Math.min(1, t));
        },
    };
}

export function ColorTween(opts) {
    let begin = opts.begin;
    let end = opts.end;
    return {
        get begin() {
            return begin;
        },
        set begin(v) {
            begin = v;
        },
        get end() {
            return end;
        },
        set end(v) {
            end = v;
        },
        lerp(t) {
            return colorLerp(begin, end, t);
        },
        transform(t) {
            return colorLerp(begin, end, Math.max(0, Math.min(1, t)));
        },
    };
}

// ---------------------------------------------------------------------------
// AnimatedContainer / AnimatedOpacity / AnimatedPositioned
// ---------------------------------------------------------------------------

// Precisely detects reactive atom handles by probing `get`, which the bridge
// validates and rejects (throws) for non-atoms. Carries the current value so
// callers avoid a second read.
function probeAtom(v) {
    if (typeof v !== "object" || v === null) return { atom: false };
    try {
        const handle = v;
        return { atom: true, handle, value: get(handle) };
    } catch {
        return { atom: false };
    }
}

// Resolve a `Val<T>` to its current static value (read once, then fixed).
function resolveStatic(v, fallback) {
    if (v == null) return fallback;
    const probe = probeAtom(v);
    return probe.atom ? probe.value : v;
}

// Register one animatable channel: returns a `derive(() => tween.lerp(progress))`
// for reactive props, or the static value unchanged for non-reactive props.
function animChannel(target, progress, makeTween, retargets, readables) {
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

function runRetargets(retargets, ctrl) {
    for (const r of retargets) r();
    ctrl.forward();
}

export function AnimatedContainer(props) {
    const duration = resolveStatic(props.duration, 300);
    const curve = resolveStatic(props.curve, "linear");
    const progress$ = source(1.0);
    const retargets = [];
    const readables = [];
    const num = (i) => Tween({ begin: i, end: i });
    // `color` props accept solid `Color`s; gradients / null snap to the new
    // target (no interpolation).
    const col = (i) => ColorTween({ begin: i, end: i });
    const ch = (v, mk) =>
        v != null
            ? animChannel(v, progress$, mk, retargets, readables)
            : undefined;

    const child = Container({
        width: ch(props.width, num),
        height: ch(props.height, num),
        padding: ch(props.padding, num),
        color: ch(props.color, col),
        borderColor: ch(props.borderColor, col),
        borderWidth: ch(props.borderWidth, num),
        borderRadius: ch(props.borderRadius, num),
        shadowColor: ch(props.shadowColor, col),
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

export function AnimatedOpacity(props) {
    const duration = resolveStatic(props.duration, 300);
    const curve = resolveStatic(props.curve, "linear");
    const progress$ = source(1.0);
    const retargets = [];
    const readables = [];
    const num = (i) => Tween({ begin: i, end: i });

    const value = animChannel(
        props.value,
        progress$,
        num,
        retargets,
        readables,
    );
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

export function AnimatedPositioned(props) {
    const duration = resolveStatic(props.duration, 300);
    const curve = resolveStatic(props.curve, "linear");
    const progress$ = source(1.0);
    const retargets = [];
    const readables = [];
    const num = (i) => Tween({ begin: i, end: i });
    const ch = (v) =>
        v != null
            ? animChannel(v, progress$, num, retargets, readables)
            : undefined;

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
