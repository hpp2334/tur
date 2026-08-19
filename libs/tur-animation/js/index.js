/**
 * @tur-ng/animation — Flutter-style animation toolkit.
 *
 * This module is the consumer-facing surface of `tur-animation`. It exposes
 * the native bridge fn (`createAnimationController`, re-exported from the
 * internal `tur:animation/native` module) and the JS-defined implicit-
 * animation widgets (`AnimatedContainer`, `AnimatedOpacity`,
 * `AnimatedPositioned`, `Tween`, `ColorTween`).
 *
 * The widgets are composed entirely from `tur:std` primitives
 * (`Tween` + `derive` + `createAnimationController`) — the elements involved
 * (`Opacity`/`Transform`/`Container`/`Positioned`) all ship as part of
 * `tur:std`.
 *
 * Each animatable prop becomes a "channel": a `derive` closure over a
 * Tween/ColorTween displayed as `tween.lerp(progress)`, where one shared
 * `AnimationController` drives a `progress` source via `onTick`. The closure
 * detects the reactive target by probing `ctx.get(target)` (which throws for
 * non-atoms, so static props pass through) and compares against the
 * last-seen target value: on change it rebases `begin` to the currently
 * displayed value, sets `end` to the new target, and restarts the
 * controller — Flutter's `ImplicitlyAnimatedWidget` retarget, inline in the
 * derive. `duration` / `curve` are static (parsed once at build).
 */

import { createAnimationController } from "tur:animation/native";

import {
    Container,
    Opacity,
    Positioned,
    colorLerp,
    derive,
    mutate,
    source,
} from "tur:std";

// `Opacity` / `Transform` are visual effects that now ship as part of
// `tur:std`; import them directly from there. This module re-exports only
// its own animation surface.
export { createAnimationController };

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

// Register one animatable channel: returns a `derive` closure that detects
// the reactive target inline — `ctx.get(target)` throws for non-atoms, so
// static props pass through unchanged. When the target's value changes, the
// closure rebases the tween (`begin` = currently-displayed value, `end` = new
// target) and restarts the shared controller, then returns the displayed
// value for this frame. No store access happens at factory/build time — the
// closure ctx is the only reactive surface.
function animChannel(target, progress, makeTween, ctrl) {
    let tween = null;
    let lastTarget;
    return derive((ctx) => {
        let v;
        try {
            v = ctx.get(target);
        } catch {
            // Not an atom handle — a static prop; never retargets.
            return target;
        }
        if (!tween) {
            tween = makeTween(v);
            lastTarget = v;
        } else if (v !== lastTarget) {
            tween.begin = tween.lerp(ctx.get(progress));
            tween.end = v;
            lastTarget = v;
            ctrl.forward();
        }
        return tween.lerp(ctx.get(progress));
    });
}

export function AnimatedContainer(props) {
    const duration = props.duration ?? 300;
    const curve = props.curve ?? "linear";
    const progress$ = source(1.0);
    // `color` props accept solid `Color`s; gradients / null snap to the new
    // target (no interpolation).
    const num = (i) => Tween({ begin: i, end: i });
    const col = (i) => ColorTween({ begin: i, end: i });

    const ctrl = createAnimationController({
        duration,
        curve,
        onTick: mutate((ctx, t) => ctx.set(progress$, t)),
        onEnd: props.onEnd,
    });

    const ch = (v, mk) => (v != null ? animChannel(v, progress$, mk, ctrl) : undefined);
    return Container({
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
}

export function AnimatedOpacity(props) {
    const duration = props.duration ?? 300;
    const curve = props.curve ?? "linear";
    const progress$ = source(1.0);
    const num = (i) => Tween({ begin: i, end: i });

    const ctrl = createAnimationController({
        duration,
        curve,
        onTick: mutate((ctx, t) => ctx.set(progress$, t)),
        onEnd: props.onEnd,
    });

    const value =
        props.value != null ? animChannel(props.value, progress$, num, ctrl) : undefined;
    return Opacity({ value, child: props.child, queryKey: props.queryKey });
}

export function AnimatedPositioned(props) {
    const duration = props.duration ?? 300;
    const curve = props.curve ?? "linear";
    const progress$ = source(1.0);
    const num = (i) => Tween({ begin: i, end: i });

    const ctrl = createAnimationController({
        duration,
        curve,
        onTick: mutate((ctx, t) => ctx.set(progress$, t)),
        onEnd: props.onEnd,
    });

    const ch = (v) => (v != null ? animChannel(v, progress$, num, ctrl) : undefined);
    return Positioned({
        left: ch(props.left),
        top: ch(props.top),
        right: ch(props.right),
        bottom: ch(props.bottom),
        width: ch(props.width),
        height: ch(props.height),
        child: props.child,
    });
}
