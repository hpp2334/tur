import { createAnimationController, Opacity } from "tur:animation";
import { type Element, mutate, set, source } from "tur:std";

// ---------------------------------------------------------------------------
// FadeIn — opacity transition used when the playground's active case content
// changes (case swap or recompile).
//
// The controller + the fadeT$ source live at module scope so they survive
// across case swaps. `triggerFadeIn()` restarts the controller from t=0;
// `FadeIn({ child })` wraps the child in `Opacity({ value: fadeT$ })` so the
// child fades in from 0 → 1 over `FADE_DURATION_MS` ms.
//
// Why this works (and didn't before):
// ------------------------------------
// `onTick` is dispatched via the engine's mutation queue, NOT synchronously
// from inside `forward()`. The old synchronous path held a `RefMut<
// AnimationController>` while firing the JS callback; the callback's `set(
// fadeT$, t)` triggered a reactive flush that re-entered the controller via
// a `downcast_ref`, panicking with boa's `BorrowError`. The new
// queue-based dispatch releases all borrows before invoking the callback,
// so the flush can safely walk the controller tree.
// ---------------------------------------------------------------------------

const FADE_DURATION_MS = 200;

const fadeT$ = source(1);

const fadeCtrl = createAnimationController({
    duration: FADE_DURATION_MS,
    curve: "easeOut",
    onTick: mutate((_ctx, t: number) => {
        set(fadeT$, t);
    }),
});

/** Restart the fade-in animation. Safe to call at any time — `stop()` first
 *  so a second rapid swap doesn't race an in-flight animation. */
export function triggerFadeIn(): void {
    fadeCtrl.stop();
    fadeCtrl.forward();
}

/** Wrap a child element so it fades in from opacity 0 → 1 over
 *  `FADE_DURATION_MS` ms whenever `triggerFadeIn()` is called. */
export function FadeIn(props: { child: Element }): Element {
    return Opacity({
        value: fadeT$,
        child: props.child,
    });
}
