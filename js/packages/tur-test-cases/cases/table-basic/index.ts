import {
    Color,
    Container,
    type Element,
    Expanded,
    mount,
    source,
    Table,
    Text,
    view,
} from "tur:std";

// Static `Table`: shared column geometry with a header row, stripes +
// dividers. Columns: a fixed 150px name column, a flex 1 notes column, and
// a flex 2 (min 120) distance column — the widths resolve against the
// available width once (CSS `table-layout: fixed` semantics), and the
// header + every row lay their cells on the same boundaries.

interface Planet {
    name: string;
    notes: string;
    distance: string;
}

// Plain constants are fine at module level (the "module level is for shared
// *state*" rule targets reactive atoms — single-view atoms live inside the
// view fn, below).
const PLANETS: Planet[] = [
    {
        name: "Mercury",
        notes: "Smallest planet, closest to the Sun",
        distance: "0.39 AU",
    },
    {
        name: "Venus",
        notes: "Hottest surface in the solar system",
        distance: "0.72 AU",
    },
    {
        name: "Earth",
        notes: "The only known home of life",
        distance: "1.00 AU",
    },
    {
        name: "Mars",
        notes: "The red planet, home to Olympus Mons",
        distance: "1.52 AU",
    },
    {
        name: "Jupiter",
        notes: "Largest planet, a gas giant",
        distance: "5.20 AU",
    },
    {
        name: "Saturn",
        notes: "Famous for its ring system",
        distance: "9.58 AU",
    },
];

/** Header label cell — one per column. */
function HeaderCell(label: string): Element {
    return Container()
        .padding(8)
        .children([
            Text({ text: label })
                .fontSize(12)
                .color(Color.hex("#94a3b8"))
                .build(),
        ])
        .build();
}

/** One body cell: padded container with left-aligned text. The Table gives
 *  cells a tight column width, so alignment inside the cell is up to the
 *  cell (wrap in your own Container with `alignment` for anything else). */
function Cell(text: string): Element {
    return Container()
        .padding(8)
        .children([
            Text({ text }).fontSize(14).color(Color.hex("#e2e8f0")).build(),
        ])
        .build();
}

const App = view(() => {
    // Local state — the view fn runs exactly once (at build), so this atom
    // is stable for the life of the tree. Static data rides a source so it
    // satisfies the Table's `rows: Readable<T[]>` contract.
    const rows$ = source(PLANETS);

    return Expanded()
        .child(
            Container()
                .color(Color.hex("#0b1220"))
                .children([
                    Table({
                        columns: [
                            { width: 150 },
                            { flex: 1 },
                            { flex: 2, minWidth: 120 },
                        ],
                        rows: rows$,
                    })
                        .queryKey(["table-basic"])
                        .headerExtent(36)
                        .rowExtent(36)
                        .stripeColor(Color.hex("#1e293b"))
                        .dividerColor(Color.hex("#334155"))
                        .dividerThickness(1)
                        .headerBuilder(() => [
                            HeaderCell("PLANET"),
                            HeaderCell("NOTES"),
                            HeaderCell("DISTANCE"),
                        ])
                        .rowBuilder((row) => [
                            Cell(row.name),
                            Cell(row.notes),
                            Cell(row.distance),
                        ])
                        .build(),
                ])
                .build(),
        )
        .build();
});

export function start() {
    mount(App);
}
