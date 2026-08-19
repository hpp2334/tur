import {
    Color,
    Container,
    derive,
    type Element,
    MouseRegion,
    mutate,
    type Point,
    PointerInteract,
    source,
} from "tur:std";
import { store } from "../state/store";
import { tokens } from "../theme/tokens";

// ---------------------------------------------------------------------------
// Per-drag transient state. `dragOwner$` is reactive (drives the highlight
// color); the start/last positions are plain closure locals — they only need
// to persist across pointer events within one drag, not trigger re-renders.
// Only one drag can be active at a time; `dragOwner$` holds the id of the
// divider that started it.
// ---------------------------------------------------------------------------

const dragOwner$ = source<number | null>(null);
let dividerCounter = 0;

/** Drag event delivered to `VDivider.onDrag`. `deltaFromStart` is the total
 *  offset from the press position; `deltaFromLast` is the offset since the
 *  previous move. Both are in canvas pixels (positive x = rightwards). */
export interface PointerDragEvent {
    deltaFromStart: Point;
    deltaFromLast: Point;
}

/**
 * A draggable vertical divider. Renders a thin strip; hovering shows a
 * `col-resize` cursor (declaratively via `MouseRegion`); pressing + dragging
 * calls `onDrag` with cumulative (`deltaFromStart`) and incremental
 * (`deltaFromLast`) pixel deltas so the parent can adjust its layout.
 *
 * The grab target is 8px wide (easy to hit); the visible bar is 1px.
 */
export function VDivider(opts: {
    onDrag: (event: PointerDragEvent) => void;
}): Element {
    const myId = ++dividerCounter;
    let dragStart: Point | null = null;
    let dragLast: Point | null = null;
    return MouseRegion({
        cursor: "col-resize",
        child: PointerInteract({
            onPointerDown: mutate((_ctx, ev) => {
                store.set(dragOwner$, myId);
                dragStart = { x: ev.global.x, y: ev.global.y };
                dragLast = { x: ev.global.x, y: ev.global.y };
            }),
            onPointerMove: mutate((_ctx, ev) => {
                if (store.get(dragOwner$) !== myId || !dragStart || !dragLast)
                    return;
                const event: PointerDragEvent = {
                    deltaFromStart: {
                        x: ev.global.x - dragStart.x,
                        y: ev.global.y - dragStart.y,
                    },
                    deltaFromLast: {
                        x: ev.global.x - dragLast.x,
                        y: ev.global.y - dragLast.y,
                    },
                };
                dragLast = { x: ev.global.x, y: ev.global.y };
                if (
                    event.deltaFromLast.x !== 0 ||
                    event.deltaFromLast.y !== 0
                ) {
                    opts.onDrag(event);
                }
            }),
            onPointerUp: mutate((_ctx, _ev) => {
                if (store.get(dragOwner$) !== myId) return;
                store.set(dragOwner$, null);
                dragStart = null;
                dragLast = null;
            }),
            child: Container({
                width: 8,
                color: derive(() =>
                    store.get(dragOwner$) === myId
                        ? Color.hex("#0ea5e922")
                        : Color.hex("#00000000"),
                ),
                children: [
                    Container({
                        width: 1,
                        color: derive(() =>
                            store.get(dragOwner$) === myId
                                ? tokens.accent.solid
                                : tokens.border.subtle,
                        ),
                    }),
                ],
            }),
        }),
    });
}
