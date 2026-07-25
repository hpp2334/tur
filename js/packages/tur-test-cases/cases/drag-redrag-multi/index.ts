import { createAnimationController, Transform } from "tur:animation";
import {
    Color,
    Container,
    derive,
    get,
    mutate,
    PointerInteract,
    Positioned,
    Stack,
    set,
    source,
    view,
} from "tur:std";

// ---------------------------------------------------------------------------
// Multi-tile drag-and-drop with a SHARED lift animation — a faithful,
// minimized repro of the jigsaw-puzzle drag mechanic.
//
//   * Two tiles laid out in a Stack via `Positioned` (reactive left/top).
//   * A SINGLE module-level `dragScale$` + `liftCtrl` drives the lift scale
//     for whichever tile is grabbed (forward on down, reverse on up) — exactly
//     like the puzzle. Grabbing tile B while tile A's `reverse()` is still
//     settling calls `forward()` on the same controller mid-flight.
//   * Drag tracking lives in plain module `let`s (same pattern as the puzzle
//     + drag-delta-tracking): handlers fire back-to-back and the reactive
//     store may not have flushed the previous write before the next handler
//     reads it.
//
// Exposes a per-tile event log + position via globalThis so the integration
// test can assert that a second drag, started right after the first release,
// still fires onPointerDown / onPointerMove on the OTHER tile.
//
// Reads use the `get(src)` import (NOT `src.get()` — a source exposes no
// `.get()` method), matching the puzzle / drag-delta-tracking convention.
// ---------------------------------------------------------------------------

const LIFT_MS = 180;
const LIFT_MAX = 1.1;
const TILE = 60;

const dragScale$ = source(1.0);
const liftCtrl = createAnimationController({
    duration: LIFT_MS,
    curve: "easeOut",
    onTick: mutate((_ctx, v: number) => {
        set(dragScale$, 1 + v * (LIFT_MAX - 1));
    }),
});

interface TileState {
    x: number;
    y: number;
}

// Each tile's position is its own source (Positioned left/top derive from it).
const tilePos$ = [
    source<TileState>({ x: 40, y: 40 }),
    source<TileState>({ x: 140, y: 40 }),
];

// Per-tile event log + the id of the actively-dragged tile. Plain module state
// (NOT reactive) for the same reason as the puzzle.
const events: string[][] = [[], []];
let dragId: number | null = null;
let lastDragId: number | null = null;
let dragOffset = { dx: 0, dy: 0 };

function readTile(id: number): TileState {
    return get(tilePos$[id]);
}

Object.assign(globalThis, {
    // "down,move,move,up" style log for tile `id`.
    __getTileEvents: (id: number): string => events[id].join(","),
    // "x,y" current position of tile `id`.
    __getTilePos: (id: number): string => {
        const p = readTile(id);
        return `${p.x},${p.y}`;
    },
    __resetDrag: (): void => {
        events[0].length = 0;
        events[1].length = 0;
        dragId = null;
    },
});

// Mirrors the puzzle's `pieceScale`: the actively-dragged tile AND the
// just-released tile (during settle) read the animated `dragScale$`; every
// other tile stays at 1.0.
function tileScale(id: number): number {
    if (dragId === id || lastDragId === id) return get(dragScale$);
    return 1;
}

function makeTile(id: number) {
    return Positioned({
        left: derive(() => readTile(id).x),
        top: derive(() => readTile(id).y),
        width: TILE,
        height: TILE,
        child: Transform({
            scale: derive(() => tileScale(id)),
            child: PointerInteract({
                onPointerDown: mutate((_ctx, ev) => {
                    const me = readTile(id);
                    dragOffset = {
                        dx: ev.global.x - me.x,
                        dy: ev.global.y - me.y,
                    };
                    dragId = id;
                    lastDragId = id;
                    events[id].push("down");
                    liftCtrl.forward();
                }),
                onPointerMove: mutate((_ctx, ev) => {
                    if (dragId !== id) return;
                    set(tilePos$[id], {
                        x: ev.global.x - dragOffset.dx,
                        y: ev.global.y - dragOffset.dy,
                    });
                    events[id].push("move");
                }),
                onPointerUp: mutate((_ctx, _ev) => {
                    if (dragId !== id) return;
                    dragId = null;
                    events[id].push("up");
                    liftCtrl.reverse();
                }),
                child: Container({
                    width: TILE,
                    height: TILE,
                    color:
                        id === 0 ? Color.hex("#6366f1") : Color.hex("#ef4444"),
                    queryKey: [`tile-${id}`],
                }),
            }),
        }),
    });
}

export default view(() =>
    Stack({
        children: [
            Container({
                width: 300,
                height: 200,
                color: Color.hex("#0f172a"),
            }),
            makeTile(0),
            makeTile(1),
        ],
    }),
);
