import {
    Axis,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    type Element,
    Expanded,
    get,
    LazyGrid,
    MainAxisSize,
    MouseRegion,
    type Mutation,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    type Readable,
    Row,
    SizedBox,
    set,
    source,
    Text,
    view,
} from "tur:std";

// ---------------------------------------------------------------------------
// "Lazy Grid Gallery" — a massive virtualized palette. 6,000 tiles, but only
// the ~20–30 inside the viewport + overscan are mounted at any time, so
// scrolling stays smooth no matter how deep you go.
//
// Demonstrates:
//   • LazyGrid virtualization (wheel-scroll rapidly — only visible rows mount)
//   • reactive maxCrossAxisExtent → column count re-derives on the fly
//   • reactive childAspectRatio → cell shape re-derives on the fly
//
// LazyGrid owns its own scroll, so it fills an Expanded pane.
// ---------------------------------------------------------------------------

const ITEM_COUNT = 6000;

// 1 = square, 2 = wide, 0.5 = tall.
const aspect$ = source<number>(1);
// Dense (more, narrower columns) vs Normal.
const dense$ = source<boolean>(false);

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
                color: derive(() =>
                    get(props.active)
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

export default view(() =>
    Container({
        color: Color.hex("#0f172a"),
        padding: 16,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Start,
                children: [
                    Text({
                        text: "Lazy Grid Gallery",
                        fontSize: 18,
                        color: Color.hex("#f1f5f9"),
                    }),
                    SizedBox({ height: 4 }),
                    Text({
                        text: `${ITEM_COUNT.toLocaleString()} tiles · only the visible rows mount — scroll to explore`,
                        fontSize: 12,
                        color: Color.hex("#94a3b8"),
                    }),
                    SizedBox({ height: 12 }),
                    Row({
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Pill({
                                label: "Square",
                                active: derive(
                                    () => Math.abs(get(aspect$) - 1) < 0.01,
                                ),
                                onClick: mutate(() => {
                                    set(aspect$, 1);
                                }),
                            }),
                            SizedBox({ width: 6 }),
                            Pill({
                                label: "Wide",
                                active: derive(() => get(aspect$) > 1),
                                onClick: mutate(() => {
                                    set(aspect$, 2);
                                }),
                            }),
                            SizedBox({ width: 6 }),
                            Pill({
                                label: "Tall",
                                active: derive(() => get(aspect$) < 1),
                                onClick: mutate(() => {
                                    set(aspect$, 0.5);
                                }),
                            }),
                        ],
                    }),
                    SizedBox({ height: 8 }),
                    Row({
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Pill({
                                label: "Normal",
                                active: derive(() => !get(dense$)),
                                onClick: mutate(() => {
                                    set(dense$, false);
                                }),
                            }),
                            SizedBox({ width: 6 }),
                            Pill({
                                label: "Dense",
                                active: derive(() => get(dense$)),
                                onClick: mutate(() => {
                                    set(dense$, true);
                                }),
                            }),
                        ],
                    }),
                    SizedBox({ height: 12 }),
                    Expanded({
                        child: Container({
                            color: Color.hex("#020617"),
                            borderRadius: 10,
                            children: [
                                LazyGrid({
                                    axis: Axis.Vertical,
                                    itemCount: ITEM_COUNT,
                                    maxCrossAxisExtent: derive(() =>
                                        get(dense$) ? 85 : 140,
                                    ),
                                    childAspectRatio: derive(() =>
                                        get(aspect$),
                                    ),
                                    crossAxisSpacing: 4,
                                    mainAxisSpacing: 4,
                                    overscan: 2,
                                    queryKey: ["lazy-grid-gallery"],
                                    builder: (i: number) => {
                                        const hue = (i * 37) % 360;
                                        return Container({
                                            color: Color.hex(
                                                hslToHex(hue, 52, 48),
                                            ),
                                            children: [
                                                Text({
                                                    text: `${i}`,
                                                    fontSize: 10,
                                                    color: Color.hex("#e2e8f0"),
                                                }),
                                            ],
                                        });
                                    },
                                }),
                            ],
                        }),
                    }),
                ],
            }),
        ],
    }),
);
