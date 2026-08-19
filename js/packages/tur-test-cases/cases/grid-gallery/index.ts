import {
    Axis,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    createStore,
    derive,
    type Element,
    Expanded,
    Grid,
    MainAxisSize,
    MouseRegion,
    type Mutation,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    type Readable,
    type ReadonlyStoreCtx,
    Row,
    ScrollView,
    SizedBox,
    source,
    Text,
    view,
} from "tur:std";
export const store = createStore();

// ---------------------------------------------------------------------------
// "Grid Gallery" — an interactive tile gallery that exercises every Grid knob:
//   • maxCrossAxisExtent → column count adapts to width + density toggle
//   • childAspectRatio → square / wide / tall cells (reactive, re-lays out)
//   • crossAxisSpacing / mainAxisSpacing → gaps
//   • click-to-select → reactive borderWidth highlight
//
// Grid is non-scrollable, so it sits inside a vertical ScrollView: the whole
// grid stays reachable regardless of the current aspect ratio.
// ---------------------------------------------------------------------------

const TILE_COUNT = 27;

// 1 = square, 16/9 ≈ 1.78 = wide, 9/16 ≈ 0.56 = tall.
const aspect$ = source<number>(1);
// Smaller maxExtent → more, narrower columns.
const maxExtent$ = source<number>(150);
// Index of the selected tile (-1 = none). Drives the white border highlight.
const selected$ = source<number>(0);

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

function Pill(props: {
    label: string;
    active: Readable<boolean>;
    onClick: Mutation<[PointerInteractEvent], void>;
}): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: props.onClick,
            child: Container({
                padding: 7,
                borderRadius: 7,
                color: derive((ctx) =>
                    ctx.get(props.active)
                        ? Color.hex("#6366f1")
                        : Color.hex("#1e293b"),
                ),
                children: [
                    Text({
                        text: props.label,
                        fontSize: 12,
                        color: Color.hex("#e2e8f0"),
                    }),
                ],
            }),
        }),
    });
}

function tile(i: number): Element {
    const hue = (i * 360) / TILE_COUNT;
    const base = Color.hex(hslToHex(hue, 58, 54));
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx) => {
                ctx.set(selected$, i);
            }),
            child: Container({
                color: base,
                borderRadius: 8,
                borderWidth: derive((ctx) =>
                    ctx.get(selected$) === i ? 3 : 0,
                ),
                borderColor: Color.hex("#ffffff"),
                children: [
                    Text({
                        text: `${i}`,
                        fontSize: 13,
                        color: Color.hex("#ffffff"),
                    }),
                ],
            }),
        }),
    });
}

function aspectLabel(ctx: ReadonlyStoreCtx): string {
    const a = ctx.get(aspect$);
    if (Math.abs(a - 1) < 0.01) return "1:1";
    if (a > 1) return "16:9";
    return "9:16";
}

export default view(() =>
    Container({
        color: Color.hex("#0f172a"),
        padding: 16,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Start,
                children: [
                    Text({
                        text: "Grid Gallery",
                        fontSize: 18,
                        color: Color.hex("#f1f5f9"),
                    }),
                    SizedBox({ height: 4 }),
                    Text({
                        text: derive(
                            (ctx) =>
                                `tile #${ctx.get(selected$)} · ${aspectLabel(ctx)} · maxExtent ${ctx.get(maxExtent$)}`,
                        ),
                        fontSize: 12,
                        color: Color.hex("#94a3b8"),
                    }),
                    SizedBox({ height: 12 }),
                    Row({
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Pill({
                                label: "1:1",
                                active: derive(
                                    (ctx) =>
                                        Math.abs(ctx.get(aspect$) - 1) < 0.01,
                                ),
                                onClick: mutate((ctx) => {
                                    ctx.set(aspect$, 1);
                                }),
                            }),
                            SizedBox({ width: 6 }),
                            Pill({
                                label: "16:9",
                                active: derive((ctx) => ctx.get(aspect$) > 1),
                                onClick: mutate((ctx) => {
                                    ctx.set(aspect$, 16 / 9);
                                }),
                            }),
                            SizedBox({ width: 6 }),
                            Pill({
                                label: "9:16",
                                active: derive((ctx) => ctx.get(aspect$) < 1),
                                onClick: mutate((ctx) => {
                                    ctx.set(aspect$, 9 / 16);
                                }),
                            }),
                        ],
                    }),
                    SizedBox({ height: 8 }),
                    Row({
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Pill({
                                label: "Dense",
                                active: derive(
                                    (ctx) => ctx.get(maxExtent$) === 95,
                                ),
                                onClick: mutate((ctx) => {
                                    ctx.set(maxExtent$, 95);
                                }),
                            }),
                            SizedBox({ width: 6 }),
                            Pill({
                                label: "Normal",
                                active: derive(
                                    (ctx) => ctx.get(maxExtent$) === 150,
                                ),
                                onClick: mutate((ctx) => {
                                    ctx.set(maxExtent$, 150);
                                }),
                            }),
                            SizedBox({ width: 6 }),
                            Pill({
                                label: "Sparse",
                                active: derive(
                                    (ctx) => ctx.get(maxExtent$) === 220,
                                ),
                                onClick: mutate((ctx) => {
                                    ctx.set(maxExtent$, 220);
                                }),
                            }),
                        ],
                    }),
                    SizedBox({ height: 12 }),
                    Expanded({
                        child: Container({
                            color: Color.hex("#020617"),
                            borderRadius: 10,
                            padding: 10,
                            children: [
                                ScrollView({
                                    axis: Axis.Vertical,
                                    child: Grid({
                                        maxCrossAxisExtent: derive((ctx) =>
                                            ctx.get(maxExtent$),
                                        ),
                                        childAspectRatio: derive((ctx) =>
                                            ctx.get(aspect$),
                                        ),
                                        crossAxisSpacing: 8,
                                        mainAxisSpacing: 8,
                                        queryKey: ["grid-gallery"],
                                        children: Array.from(
                                            { length: TILE_COUNT },
                                            (_, i) => tile(i),
                                        ),
                                    }),
                                }),
                            ],
                        }),
                    }),
                ],
            }),
        ],
    }),
);
