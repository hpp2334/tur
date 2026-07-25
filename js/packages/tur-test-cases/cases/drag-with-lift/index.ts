import { createAnimationController, Transform } from "tur:animation";
import {
    Color,
    Container,
    derive,
    get,
    mutate,
    PointerInteract,
    set,
    source,
    Text,
    view,
} from "tur:std";

// ---------------------------------------------------------------------------
// Drag-and-drop with a lift animation — mirrors the jigsaw-puzzle's drag
// mechanic: on pointer-down a `createAnimationController` runs `forward()`
// (scale 1 → 1.1); on pointer-up it runs `reverse()`. The drag itself tracks
// the pointer via module state (same pattern as drag-delta-tracking and the
// puzzle). Exposes the last pointer event seen via globalThis so the
// integration test can assert that a second drag, started right after the
// first release, still fires onPointerDown / onPointerMove.
//
// This fixture exists to reproduce the jigsaw symptom ("after dragging +
// releasing a tile, no tile can be dragged again for ~1-2s") in isolation.
// ---------------------------------------------------------------------------

const LIFT_MS = 180;
const LIFT_MAX = 1.1;
const dragScale$ = source(1.0);
const liftCtrl = createAnimationController({
    duration: LIFT_MS,
    curve: "easeOut",
    onTick: mutate((_ctx, v: number) => {
        set(dragScale$, 1 + v * (LIFT_MAX - 1));
    }),
});

let lastEvent = "idle";
let dragStart: { x: number; y: number } | null = null;

Object.assign(globalThis, {
    __getLastEvent: (): string => lastEvent,
    __resetDrag: (): void => {
        lastEvent = "idle";
        dragStart = null;
    },
});

export default view(() =>
    Transform({
        scale: derive(() => get(dragScale$)),
        child: PointerInteract({
            onPointerDown: mutate((_ctx, ev) => {
                dragStart = { x: ev.global.x, y: ev.global.y };
                lastEvent = "down";
                liftCtrl.forward();
            }),
            onPointerMove: mutate((_ctx, _ev) => {
                if (!dragStart) return;
                lastEvent = "move";
            }),
            onPointerUp: mutate((_ctx, _ev) => {
                dragStart = null;
                lastEvent = "up";
                liftCtrl.reverse();
            }),
            child: Container({
                width: 200,
                height: 200,
                color: Color.hex("#6366f1"),
                queryKey: ["lift-target"],
                children: [Text({ text: "drag me" })],
            }),
        }),
    }),
);
