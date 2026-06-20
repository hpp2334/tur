import {
    Color,
    Container,
    derive,
    type EdgyElement,
    get,
    MouseRegion,
    mutate,
    PointerInteract,
    set,
    source,
} from "@tur/edgy";
import { tokens } from "../theme/tokens";

// ---------------------------------------------------------------------------
// Per-drag transient state. Held at module scope so it survives tree rebuilds
// (a drag in progress is not interrupted by reactive updates). Only one drag
// can be active at a time — the divider that started the drag owns it.
// ---------------------------------------------------------------------------

const dragActive$ = source(false);
const dragStartX$ = source(0);

function resetDrag(): void {
    set(dragActive$, false);
    set(dragStartX$, 0);
}

/**
 * A draggable vertical divider. Renders a thin strip; hovering shows a
 * `col-resize` cursor (declaratively via `MouseRegion`); pressing + dragging
 * calls `onDrag` with each pixel delta (positive = right) so the parent can
 * adjust its layout.
 *
 * The grab target is 8px wide (easy to hit); the visible bar is 1px.
 */
export function VDivider(opts: {
    onDrag: (dx: number) => void;
}): EdgyElement {
    return MouseRegion({
        cursor: "col-resize",
        child: PointerInteract({
            onPointerDown: mutate((_ctx, ev) => {
                set(dragStartX$, ev.global.x);
                set(dragActive$, true);
            }),
            onPointerMove: mutate((_ctx, ev) => {
                if (!get(dragActive$)) return;
                const start = get(dragStartX$);
                const dx = ev.global.x - start;
                if (dx !== 0) {
                    set(dragStartX$, ev.global.x);
                    opts.onDrag(dx);
                }
            }),
            onPointerUp: mutate((_ctx, _ev) => {
                resetDrag();
            }),
            child: Container({
                width: 8,
                color: derive(() =>
                    get(dragActive$)
                        ? Color.hex("#0ea5e922")
                        : Color.hex("#00000000"),
                ),
                children: [
                    Container({
                        width: 1,
                        color: derive(() =>
                            get(dragActive$)
                                ? tokens.accent.solid
                                : tokens.border.subtle,
                        ),
                    }),
                ],
            }),
        }),
    });
}
