import {
    Alignment,
    Axis,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    Each,
    type Element,
    Expanded,
    LazyList,
    MainAxisSize,
    MouseRegion,
    mount,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    type Readable,
    Row,
    SizedBox,
    source,
    Text,
    view,
} from "tur:std";

// ---------------------------------------------------------------------------
// "Variable width + height LazyList" — items with DIFFERENT main-axis extents
// (heights in vertical mode, widths in horizontal mode). A toggle button at
// the top flips the axis at runtime via an `Each`-keyed rebuild — the same
// "rebuild-on-change" idiom used in
// demo/playground-view/src/views/viewer.ts.
//
// Engine notes:
//   * When `itemExtent` is OMITTED, LazyList measures each visible item,
//     caches its main-axis extent per logical index, and positions items
//     via an anchor-and-walk from a persistent `first_mounted_offset`
//     (delta-maintained in `process_remount`). Layout is O(visible_count)
//     regardless of scroll depth — see
//     libs/tur-engine/src/elements/lazy_list/element.rs and
//     lazy_list/render.rs. `overscan: 2` absorbs measurement timing.
//   * Cross-axis sizes are CLAMPED to the viewport (lazy_list/render.rs:
//     34-47): a vertical list cannot have rows of different widths. We
//     therefore show "different widths" via the inner colored bar, whose
//     width varies per item inside the (full-width) row.
// ---------------------------------------------------------------------------

const ITEM_COUNT = 2000;

// Deterministic main-axis size via a sine-based hash → integer in [1, 150].
// Every index maps to a pseudo-random but stable extent, so consecutive
// rows/cols look visibly different and there's no short cycle to spot.
// The engine maintains a persistent per-index extent cache (see
// libs/tur-engine/src/elements/lazy_list/element.rs:`extent_cache`) and
// positions each item at the cumulative offset of its logical index — so
// even a wide [30, 180] variance produces a tight, non-overlapping column
// as the user scrolls.
function mainSizeFor(i: number): number {
    const frac = Math.sin(i * 12.9898) * 43758.5453;
    return 30 + Math.floor((frac - Math.floor(frac)) * 151);
}

// Inner content cross-axis size — proves "different widths" (vertical) or
// "different heights" (horizontal) inside each clamped row/column.
function crossSizeFor(i: number): number {
    return 60 + ((i * 37) % 200);
}

function hueFor(i: number): number {
    return (i * 47) % 360;
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

function buildItem(i: number, axis: Axis): Element {
    const main = mainSizeFor(i);
    const cross = crossSizeFor(i);
    const hue = hueFor(i);
    const barColor = Color.hex(hslToHex(hue, 65, 55));
    const zebra = i % 2 === 0 ? Color.hex("#0f172a") : Color.hex("#1e293b");
    const labelColor = Color.hex("#cbd5e1");

    if (axis === Axis.Vertical) {
        // Row spans full viewport width (clamped); row height = `main`.
        // Inner bar of width `cross` proves "different widths" per row.
        const barH = Math.min(main - 16, 26);
        return Container({
            height: main,
            color: zebra,
            padding: 8,
            alignment: Alignment.CenterLeft,
            children: [
                Row({
                    mainAxisSize: MainAxisSize.Min,
                    children: [
                        Container({
                            width: cross,
                            height: barH,
                            color: barColor,
                            borderRadius: 4,
                        }),
                        SizedBox({ width: 10 }),
                        Text({
                            text: `#${i}  h=${main}  bar=${cross}`,
                            fontSize: 11,
                            color: labelColor,
                        }),
                    ],
                }),
            ],
        });
    }

    // Horizontal: column spans full viewport height (clamped); column
    // width = `main`. Inner bar of height `cross` proves "different heights".
    const barW = Math.min(main - 16, 26);
    return Container({
        width: main,
        color: zebra,
        padding: 8,
        alignment: Alignment.TopCenter,
        children: [
            Column({
                mainAxisSize: MainAxisSize.Min,
                children: [
                    Container({
                        width: barW,
                        height: cross,
                        color: barColor,
                        borderRadius: 4,
                    }),
                    SizedBox({ height: 8 }),
                    Text({
                        text: `#${i}`,
                        fontSize: 11,
                        color: labelColor,
                    }),
                    Text({
                        text: `w=${main}`,
                        fontSize: 10,
                        color: Color.hex("#64748b"),
                    }),
                ],
            }),
        ],
    });
}

// Takes the reactive axis as a parameter (local state owned by the view fn).
function ToggleButton(axis$: Readable<Axis>): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx, _ev: PointerInteractEvent) => {
                const cur = ctx.get(axis$);
                ctx.set(
                    axis$,
                    cur === Axis.Vertical ? Axis.Horizontal : Axis.Vertical,
                );
            }),
            child: Container({
                padding: 10,
                borderRadius: 8,
                color: Color.hex("#4f46e5"),
                children: [
                    Text({
                        text: derive((ctx) =>
                            ctx.get(axis$) === Axis.Vertical
                                ? "Axis: Vertical  (click to flip)"
                                : "Axis: Horizontal  (click to flip)",
                        ),
                        fontSize: 13,
                        color: Color.hex("#ffffff"),
                    }),
                ],
            }),
        }),
    });
}

const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so this atom is
    // stable for the life of the tree. Helpers that need it (ToggleButton)
    // take the atom as a parameter — no module-level hoisting required.
    // Toggling flips vertical ↔ horizontal and rebuilds the list.
    const axis$ = source<Axis>(Axis.Vertical);

    return Expanded({
        child: Container({
            color: Color.hex("#020617"),
            padding: 16,
            children: [
                Column({
                    crossAlignment: CrossAxisAlignment.Start,
                    children: [
                        ToggleButton(axis$),
                        SizedBox({ height: 12 }),
                        Expanded({
                            child: Container({
                                color: Color.hex("#0b1220"),
                                borderRadius: 8,
                                children: [
                                    // Each rebuilds its single child when axis$
                                    // changes — re-mounting the LazyList with
                                    // the new axis. This is the canonical
                                    // rebuild-on-change idiom in this codebase.
                                    Each({
                                        items: derive((ctx) => [
                                            ctx.get(axis$),
                                        ]),
                                        build: (axis: Axis) =>
                                            LazyList({
                                                axis,
                                                itemCount: ITEM_COUNT,
                                                overscan: 2,
                                                queryKey: [
                                                    "lazy-list-var-sizes",
                                                    axis === Axis.Vertical
                                                        ? "v"
                                                        : "h",
                                                ],
                                                builder: (i: number) =>
                                                    buildItem(i, axis),
                                            }),
                                    }),
                                ],
                            }),
                        }),
                    ],
                }),
            ],
        }),
    });
});

export function start() {
    mount(App);
}
