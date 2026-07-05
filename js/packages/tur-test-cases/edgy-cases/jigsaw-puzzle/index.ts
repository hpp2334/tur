import {
    Alignment,
    type AnimationController,
    Color,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    createAnimationController,
    derive,
    type Element,
    Expanded,
    get,
    MainAxisAlignment,
    MouseRegion,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    Positioned,
    Row,
    Stack,
    set,
    source,
    Text,
    Transform,
    view,
} from "builtin:tur/std";

// ---------------------------------------------------------------------------
// "Jigsaw puzzle" — a 3x3 drag-and-drop game.
//
//   * Nine colored pieces start in the right-hand tray (shuffled).
//   * Drag any piece onto the left-hand board. Drop within SNAP_THRESHOLD of
//     its correct slot and it snaps + locks in place.
//   * A HUD (top-right) tracks X/9 pieces placed; a "Solved!" banner appears
//     when all nine are locked.
//
// Drag mechanics:
//   * Each piece is a `Positioned` (reactive `left`/`top`) wrapping a
//     `PointerInteract`. Positions derive from the shared `pieces$` atom.
//   * The engine implements gesture capture (core/gesture/mod.rs:24-64 and
//     handlers/gesture.rs:77-94): once a pointer-down hits a piece, that
//     piece keeps receiving move/up events for the entire drag even if the
//     pointer leaves its bounds. Fast drags don't drop events.
//   * The 9 pieces are mounted ONCE via a static Stack children array (NOT
//     via `Each`, which would tear down + rebuild every piece on each move
//     event — losing pointer capture mid-drag). Only each piece's `left`/`top`
//     derive re-evaluates as `pieces$` mutates.
//   * `Positioned` is fully reactive (libs/tur-engine/src/elements/positioned/
//     render.rs:10-21 + layout/layout_context.rs:123-134): changing `left`/
//     `top` re-layouts the parent Stack and redraws next frame.
// ---------------------------------------------------------------------------

const GRID = 3;
const CELL = 60;
const GAP = 4;
const PIECE = CELL - GAP; // 56
const BOARD_W = GRID * CELL; // 180
// Layout: board on top, tray below — both centered horizontally. The viewer
// pane is ~400px wide, so a side-by-side board+tray (needs 600px+) doesn't
// fit. Vertical stacking keeps everything on-screen.
const BOARD_X = 100;
const BOARD_Y = 60;
const TRAY_X = 100;
const TRAY_Y = BOARD_Y + BOARD_W + 60; // 300
const SNAP_THRESHOLD = 28;
const PLAY_W = 380;
const PLAY_H = TRAY_Y + BOARD_W + 40; // 520

interface Piece {
    id: number;
    slot: number; // target slot 0..8 on the board
    x: number;
    y: number;
    placed: boolean;
}

function slotToPos(
    slot: number,
    originX: number,
    originY: number,
): { x: number; y: number } {
    return {
        x: originX + (slot % GRID) * CELL + GAP / 2,
        y: originY + Math.floor(slot / GRID) * CELL + GAP / 2,
    };
}

function shuffle<T>(arr: T[]): T[] {
    const a = arr.slice();
    for (let i = a.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        const tmp = a[i];
        a[i] = a[j];
        a[j] = tmp;
    }
    return a;
}

function initialPieces(): Piece[] {
    // Each piece's `slot` is its TARGET on the board (a shuffled permutation
    // of 0..8). Its initial position is its slot in the TRAY grid keyed by
    // `id`, so the tray shows the pieces in id order while their targets are
    // shuffled — the user has to read each piece's number to find its slot.
    const slots = shuffle([0, 1, 2, 3, 4, 5, 6, 7, 8]);
    return slots.map((slot, id) => {
        const trayPos = slotToPos(id, TRAY_X, TRAY_Y);
        return { id, slot, x: trayPos.x, y: trayPos.y, placed: false };
    });
}

function hslToHex(h: number, s: number, l: number): string {
    const sat = s / 100;
    const light = l / 100;
    const c = (1 - Math.abs(2 * light - 1)) * sat;
    const hp = h / 60;
    const x = c * (1 - Math.abs((hp % 2) - 1));
    let r1 = 0,
        g1 = 0,
        b1 = 0;
    if (hp < 1) {
        r1 = c;
        g1 = x;
    } else if (hp < 2) {
        r1 = x;
        g1 = c;
    } else if (hp < 3) {
        g1 = c;
        b1 = x;
    } else if (hp < 4) {
        g1 = x;
        b1 = c;
    } else if (hp < 5) {
        r1 = x;
        b1 = c;
    } else {
        r1 = c;
        b1 = x;
    }
    const m = light - c / 2;
    const r = Math.round((r1 + m) * 255);
    const g = Math.round((g1 + m) * 255);
    const b = Math.round((b1 + m) * 255);
    return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

function pieceColor(slot: number, placed: boolean): Color {
    const hue = (slot * 47) % 360;
    // Placed pieces are dimmer (lower saturation/lightness) so the user can
    // tell at a glance which pieces are locked in.
    return Color.hex(hslToHex(hue, placed ? 35 : 60, placed ? 42 : 55));
}

// Raw RGB triple for `slot`, reused for colored glows/shadows (Container's
// `shadowColor` wants an explicit Color value, not a derive of a hex string).
function pieceRgb(slot: number): { r: number; g: number; b: number } {
    const hue = (slot * 47) % 360;
    const hex = hslToHex(hue, 60, 55);
    return {
        r: Number.parseInt(hex.slice(1, 3), 16),
        g: Number.parseInt(hex.slice(3, 5), 16),
        b: Number.parseInt(hex.slice(5, 7), 16),
    };
}

// --- Drag-lift animation ---------------------------------------------------
//
// Game-feel: when the user grabs a piece it scales up to LIFT_MAX (1.1)
// over ~180 ms (easeOut). On release it settles back to 1.0 at the same
// speed. A single AnimationController drives the shared `dragScale$` source;
// only the actively-dragged piece (or the just-released piece during settle)
// reads it — all other pieces stay at scale 1.0.

const LIFT_MAX = 1.1;
const LIFT_MS = 180;
const dragScale$ = source(1.0);
const liftCtrl: AnimationController = createAnimationController({
    duration: LIFT_MS,
    curve: "easeOut",
    onTick: mutate((_ctx, v: number) => {
        set(dragScale$, 1 + v * (LIFT_MAX - 1));
    }),
});

// --- Reactive state --------------------------------------------------------

const pieces$ = source<Piece[]>(initialPieces());
const placedCount$ = derive(() => get(pieces$).filter((p) => p.placed).length);
const done$ = derive(() => get(placedCount$) === GRID * GRID);

// Drag tracking lives in plain module state, NOT reactive sources. Mutation
// handlers fire synchronously back-to-back (down → move → move → … → up) and
// the engine's reactive store may not have flushed the previous handler's
// write by the time the next handler calls `ctx.get` — that would make the
// move handler see a stale `dragId` and abort the drag. Module `let` writes
// are visible to the next handler immediately. (Same pattern as
// drag-delta-tracking/index.ts.) `pieces$` stays reactive because the
// Positioned `left`/`top` derives need to re-evaluate when a piece moves.
let dragId: number | null = null;
let lastDragId: number | null = null;
let dragOffset = { dx: 0, dy: 0 };

// Returns the current scale for piece `id`: the animated `dragScale$` value
// for the actively-dragged piece AND the just-released piece (during settle),
// 1.0 for everything else.
function pieceScale(id: number): number {
    const scale = get(dragScale$);
    if (dragId === id || lastDragId === id) return scale;
    return 1;
}

function pieceDragging(id: number): boolean {
    return (dragId === id || lastDragId === id) && get(dragScale$) > 1.001;
}

// --- Per-piece drag handlers (close over `id`) -----------------------------

const onPieceDown = (id: number) =>
    mutate((ctx, ev: PointerInteractEvent) => {
        const me = ctx.get(pieces$)[id];
        if (!me || me.placed) return;
        // Record offset between pointer and piece top-left so the piece
        // doesn't jump to the pointer on the first move event.
        dragOffset = { dx: ev.global.x - me.x, dy: ev.global.y - me.y };
        dragId = id;
        lastDragId = id;
        liftCtrl.forward();
    });

const onPieceMove = (id: number) =>
    mutate((ctx, ev: PointerInteractEvent) => {
        if (dragId !== id) return;
        const nx = ev.global.x - dragOffset.dx;
        const ny = ev.global.y - dragOffset.dy;
        const cur = ctx.get(pieces$);
        ctx.set(
            pieces$,
            cur.map((p) => (p.id === id ? { ...p, x: nx, y: ny } : p)),
        );
    });

const onPieceUp = (id: number) =>
    mutate((ctx, _ev: PointerInteractEvent) => {
        if (dragId !== id) return;
        const cur = ctx.get(pieces$);
        const me = cur[id];
        if (me) {
            const target = slotToPos(me.slot, BOARD_X, BOARD_Y);
            const dist = Math.hypot(me.x - target.x, me.y - target.y);
            if (dist < SNAP_THRESHOLD) {
                // Snap + lock.
                ctx.set(
                    pieces$,
                    cur.map((p) =>
                        p.id === id
                            ? { ...p, x: target.x, y: target.y, placed: true }
                            : p,
                    ),
                );
            }
            // Else: leave the piece where it was dropped (re-grabbable).
        }
        dragId = null;
        liftCtrl.reverse();
    });

const resetPuzzle = mutate((ctx, _ev: PointerInteractEvent) => {
    ctx.set(pieces$, initialPieces());
    dragId = null;
    lastDragId = null;
    liftCtrl.stop();
    set(dragScale$, 1);
});

// --- View helpers ----------------------------------------------------------

function pieceById(id: number): Piece {
    // `pieces$` is created in id order (initialPieces uses `.map((slot, id))`)
    // and every mutation uses `.map((p) => p.id === id ? ... : p)` which
    // preserves array order — so indexing by id is always correct.
    return get(pieces$)[id];
}

function makePiece(id: number): Element {
    return Positioned({
        left: derive(() => pieceById(id).x),
        top: derive(() => pieceById(id).y),
        width: PIECE,
        height: PIECE,
        child: Transform({
            scale: derive(() => pieceScale(id)),
            child: PointerInteract({
                onPointerDown: onPieceDown(id),
                onPointerMove: onPieceMove(id),
                onPointerUp: onPieceUp(id),
                child: Container({
                    width: PIECE,
                    height: PIECE,
                    color: derive(() => {
                        const me = pieceById(id);
                        return pieceColor(me.slot, me.placed);
                    }),
                    borderRadius: 14,
                    borderColor: Color.hex("#ffffff"),
                    borderWidth: 2,
                    // Placed pieces glow in their own hue; unplaced pieces
                    // cast a soft neutral shadow. Dragged pieces cast a
                    // stronger, deeper shadow to reinforce the lift.
                    shadowColor: derive(() => {
                        const me = pieceById(id);
                        if (pieceDragging(id)) return Color.rgba(0, 0, 0, 180);
                        if (!me.placed) return Color.rgba(0, 0, 0, 110);
                        const c = pieceRgb(me.slot);
                        return Color.rgba(c.r, c.g, c.b, 140);
                    }),
                    shadowOffset: derive(() =>
                        pieceDragging(id) ? [0, 12] : [0, 4],
                    ),
                    shadowBlur: derive(() => {
                        const me = pieceById(id);
                        if (pieceDragging(id)) return 28;
                        return me.placed ? 18 : 10;
                    }),
                    alignment: Alignment.Center,
                    children: [
                        Text({
                            text: derive(() => `${pieceById(id).slot + 1}`),
                            fontSize: 28,
                            color: Color.hex("#ffffff"),
                        }),
                    ],
                }),
            }),
        }),
    });
}

function slotGhost(slot: number): Element {
    const pos = slotToPos(slot, BOARD_X, BOARD_Y);
    return Positioned({
        left: pos.x,
        top: pos.y,
        width: PIECE,
        height: PIECE,
        child: Container({
            width: PIECE,
            height: PIECE,
            color: Color.hex("#1e293b"),
            borderColor: Color.hex("#475569"),
            borderWidth: 1,
            borderRadius: 12,
            alignment: Alignment.Center,
            children: [
                Text({
                    text: `${slot + 1}`,
                    fontSize: 18,
                    color: Color.hex("#64748b"),
                }),
            ],
        }),
    });
}

function BoardBackground(): Element {
    return Positioned({
        left: BOARD_X - GAP / 2,
        top: BOARD_Y - GAP / 2,
        width: BOARD_W,
        height: BOARD_W,
        child: Container({
            width: BOARD_W,
            height: BOARD_W,
            color: Color.hex("#0f172a"),
            borderColor: Color.hex("#3b82f6"),
            borderWidth: 2,
            borderRadius: 16,
            shadowColor: Color.rgba(59, 130, 246, 40),
            shadowOffset: [0, 0],
            shadowBlur: 18,
        }),
    });
}

function TrayBackground(): Element {
    return Positioned({
        left: TRAY_X - GAP / 2,
        top: TRAY_Y - GAP / 2,
        width: BOARD_W,
        height: BOARD_W,
        child: Container({
            width: BOARD_W,
            height: BOARD_W,
            color: Color.hex("#0a0f1d"),
            borderColor: Color.hex("#334155"),
            borderWidth: 1,
            borderRadius: 16,
        }),
    });
}

function TopBar(): Element {
    // Single Positioned spans the full width; Row with SpaceBetween pushes
    // Shuffle to the left and the HUD counter to the right. (Positioned with
    // only `right` set is not honored by the engine — needs `left` too, so
    // we anchor the whole bar with both and let the Row distribute.)
    return Positioned({
        left: 12,
        right: 12,
        top: 12,
        child: Row({
            mainAlignment: MainAxisAlignment.SpaceBetween,
            crossAlignment: CrossAxisAlignment.Center,
            children: [
                MouseRegion({
                    cursor: "pointer",
                    child: PointerInteract({
                        onClick: resetPuzzle,
                        child: Container({
                            padding: 10,
                            borderRadius: 20,
                            color: Color.hex("#4f46e5"),
                            borderColor: Color.hex("#818cf8"),
                            borderWidth: 1,
                            shadowColor: Color.rgba(79, 70, 229, 120),
                            shadowOffset: [0, 4],
                            shadowBlur: 12,
                            children: [
                                Text({
                                    text: "Shuffle",
                                    fontSize: 13,
                                    color: Color.hex("#ffffff"),
                                }),
                            ],
                        }),
                    }),
                }),
                Container({
                    padding: 10,
                    borderRadius: 8,
                    color: Color.hex("#1e293b"),
                    borderColor: Color.hex("#334155"),
                    borderWidth: 1,
                    children: [
                        Text({
                            text: derive(
                                () => `${get(placedCount$)} / 9 placed`,
                            ),
                            fontSize: 14,
                            color: Color.hex("#e2e8f0"),
                        }),
                    ],
                }),
            ],
        }),
    });
}

function WinBanner(): Element {
    return Positioned({
        left: 0,
        right: 0,
        top: 0,
        bottom: 0,
        child: Container({
            color: Color.rgba(2, 6, 23, 200),
            alignment: Alignment.Center,
            children: [
                Container({
                    padding: 24,
                    borderRadius: 18,
                    color: Color.hex("#4f46e5"),
                    borderColor: Color.hex("#818cf8"),
                    borderWidth: 2,
                    shadowColor: Color.rgba(79, 70, 229, 180),
                    shadowOffset: [0, 8],
                    shadowBlur: 32,
                    children: [
                        Text({
                            text: "Solved!",
                            fontSize: 36,
                            color: Color.hex("#ffffff"),
                        }),
                    ],
                }),
            ],
        }),
    });
}

export default view(() =>
    // Fill the entire viewer pane: an outer Stack provides the full-bleed
    // dark background + a centered puzzle play area + the win banner overlay.
    // The puzzle itself is a fixed-size Stack (PLAY_W × PLAY_H) so the
    // Positioned piece/slot coordinates remain case-local; the surrounding
    // Column centers it within whatever size the viewer pane happens to be.
    Expanded({
        child: Stack({
            children: [
                // Full-bleed background — fills the viewer regardless of
                // puzzle dimensions.
                Container({
                    color: Color.hex("#020617"),
                }),
                // Centered puzzle. Column with MainAxisSize.Max fills the
                // parent Container so the centering has the full viewer to
                // center against; cross-axis Center handles horizontal.
                Container({
                    children: [
                        Column({
                            mainAlignment: MainAxisAlignment.Center,
                            crossAlignment: CrossAxisAlignment.Center,
                            children: [
                                Stack({
                                    children: [
                                        // Sizer — gives the inner Stack a
                                        // finite size so Positioned children
                                        // resolve against it.
                                        Container({
                                            width: PLAY_W,
                                            height: PLAY_H,
                                        }),
                                        BoardBackground(),
                                        ...Array.from(
                                            { length: GRID * GRID },
                                            (_, i) => slotGhost(i),
                                        ),
                                        TrayBackground(),
                                        ...Array.from(
                                            { length: GRID * GRID },
                                            (_, id) => makePiece(id),
                                        ),
                                        TopBar(),
                                    ],
                                }),
                            ],
                        }),
                    ],
                }),
                // Win banner overlays the FULL viewer (not just the puzzle
                // area), so it can stay at its natural size without
                // obscuring just one corner.
                Condition({
                    condition: done$,
                    child: () => WinBanner(),
                }),
            ],
        }),
    }),
);
