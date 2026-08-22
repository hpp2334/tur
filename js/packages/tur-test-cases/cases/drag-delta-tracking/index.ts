import { Color, Container, mutate, PointerInteract, view } from "tur:std";

// Mirrors the drag-delta tracking logic in the playground's VDivider:
// tracks the press position (dragStart) and the previous move position
// (dragLast), then computes deltaFromStart and deltaFromLast. Exposes the
// last computed deltas to Rust via globalThis so the integration test can
// verify exact values after each pointer move.
let dragStart: { x: number; y: number } | null = null;
let dragLast: { x: number; y: number } | null = null;
let lastInfo = { dsx: 0, dsy: 0, dlx: 0, dly: 0 };

Object.assign(globalThis, {
    __getDragInfo: (): string =>
        `${lastInfo.dsx},${lastInfo.dsy},${lastInfo.dlx},${lastInfo.dly}`,
    __resetDrag: (): void => {
        dragStart = null;
        dragLast = null;
        lastInfo = { dsx: 0, dsy: 0, dlx: 0, dly: 0 };
    },
});

export default view(() =>
    PointerInteract({
        onPointerDown: mutate((_ctx, ev) => {
            dragStart = { x: ev.global.x, y: ev.global.y };
            dragLast = { x: ev.global.x, y: ev.global.y };
        }),
        onPointerMove: mutate((_ctx, ev) => {
            if (!dragStart || !dragLast) return;
            lastInfo = {
                dsx: ev.global.x - dragStart.x,
                dsy: ev.global.y - dragStart.y,
                dlx: ev.global.x - dragLast.x,
                dly: ev.global.y - dragLast.y,
            };
            dragLast = { x: ev.global.x, y: ev.global.y };
        }),
        onPointerUp: mutate((_ctx, _ev) => {
            dragStart = null;
            dragLast = null;
        }),
        child: Container({
            width: 200,
            height: 200,
            color: Color.hex("#6366f1"),
            queryKey: ["drag-target"],
        }),
    }),
);
