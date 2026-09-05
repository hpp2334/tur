import { createAnimationController } from "tur:animation";
import {
    Color,
    Container,
    derive,
    mount,
    mutate,
    PointerInteract,
    Positioned,
    type ReadonlyStoreCtx,
    Stack,
    type Store,
    source,
    Transform,
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
// Reads flow through the closure ctx (`ctx.get(src)`) or the instance store
// captured by the test hooks registered in `start`, matching the puzzle /
// drag-delta-tracking convention.
//
// Module-level on purpose throughout: the atoms/`let`s are shared across the
// module helpers (`readTile`/`tileScale`/`makeTile`) AND the test seam in
// `start` — not because view fns re-run (they run exactly once, at build).
// ---------------------------------------------------------------------------

const LIFT_MS = 180;
const LIFT_MAX = 1.1;
const TILE = 60;

const dragScale$ = source(1.0);
const liftCtrl = createAnimationController({
    duration: LIFT_MS,
    curve: "easeOut",
    onTick: mutate((ctx, v: number) => {
        ctx.set(dragScale$, 1 + v * (LIFT_MAX - 1));
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

function readTile(r: ReadonlyStoreCtx, id: number): TileState {
    return r.get(tilePos$[id]);
}

// Mirrors the puzzle's `pieceScale`: the actively-dragged tile AND the
// just-released tile (during settle) read the animated `dragScale$`; every
// other tile stays at 1.0.
function tileScale(r: ReadonlyStoreCtx, id: number): number {
    if (dragId === id || lastDragId === id) return r.get(dragScale$);
    return 1;
}

function makeTile(id: number) {
    return Positioned({
        left: derive((ctx) => readTile(ctx, id).x),
        top: derive((ctx) => readTile(ctx, id).y),
        width: TILE,
        height: TILE,
        child: Transform({
            scale: derive((ctx) => tileScale(ctx, id)),
            child: PointerInteract({
                onPointerDown: mutate((ctx, ev) => {
                    const me = readTile(ctx, id);
                    dragOffset = {
                        dx: ev.global.x - me.x,
                        dy: ev.global.y - me.y,
                    };
                    dragId = id;
                    lastDragId = id;
                    events[id].push("down");
                    liftCtrl.forward();
                }),
                onPointerMove: mutate((ctx, ev) => {
                    if (dragId !== id) return;
                    ctx.set(tilePos$[id], {
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

const App = view(() =>
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

export function start({ store }: { store: Store }) {
    Object.assign(globalThis, {
        // "down,move,move,up" style log for tile `id`.
        __getTileEvents: (id: number): string => events[id].join(","),
        // "x,y" current position of tile `id`.
        __getTilePos: (id: number): string => {
            const p = readTile(store, id);
            return `${p.x},${p.y}`;
        },
        __resetDrag: (): void => {
            events[0].length = 0;
            events[1].length = 0;
            dragId = null;
        },
    });
    mount(App);
}
