import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    type Derived,
    derive,
    type Element,
    Expanded,
    lifecycleView,
    MouseRegion,
    type Mutation,
    mount,
    mutate,
    PointerInteract,
    type ReadonlyStoreCtx,
    SizedBox,
    type Source,
    sleep,
    source,
    Table,
    Text,
    view,
} from "tur:std";

// Reactive `Table` with async data: rows arrive from `getFakeData()` — a
// fake API that resolves after a 300ms delay. Loading starts at mount
// (`lifecycleView`'s `onMounted$`), the table renders its header with an
// empty body while `data$` is null, and the loaded rows flow in through the
// derived `rows$` (sorted by the current header state — a fresh array per
// change is what triggers the Table's row rebuild; in-place mutation of the
// same array object is never observed, same rule as `Each`). Header labels
// react through `Val` props inside the once-built `buildHeader` cells.

type SortKey = "name" | "moons" | "gravity";

interface Row {
    name: string;
    moons: number;
    gravity: number;
}

/** Simulated fetch: 300ms latency, then the rows. The delay rides the
 *  engine's frame-precise `sleep` (there is no `setTimeout`). */
async function getFakeData(): Promise<Row[]> {
    await sleep(300).promise;
    return [
        { name: "Mercury", moons: 0, gravity: 3.7 },
        { name: "Venus", moons: 0, gravity: 8.9 },
        { name: "Earth", moons: 1, gravity: 9.8 },
        { name: "Mars", moons: 2, gravity: 3.7 },
        { name: "Jupiter", moons: 95, gravity: 24.8 },
        { name: "Saturn", moons: 146, gravity: 10.4 },
        { name: "Uranus", moons: 28, gravity: 8.9 },
        { name: "Neptune", moons: 16, gravity: 11.2 },
    ];
}

const COLS: { key: SortKey; label: string }[] = [
    { key: "name", label: "PLANET" },
    { key: "moons", label: "MOONS" },
    { key: "gravity", label: "GRAVITY (m/s²)" },
];

// ── table state bundle ────────────────────────────────────────────────────
// One object passed to the helper factories that need it — the countdown /
// jigsaw-puzzle pattern for state shared between a view's helpers.

interface TableState {
    rows$: Derived<Row[]>;
    loading$: Derived<boolean>;
    load$: Mutation<[], void>;
    abort$: Mutation<[], void>;
    sortKey$: Source<SortKey>;
    sortDir$: Source<number>;
    sortBy$: Mutation<[SortKey], void>;
}

function createTableState(): TableState {
    // ── sort state ──
    const sortKey$ = source<SortKey>("moons");
    const sortDir$ = source(1);

    const sortBy$ = mutate((ctx, key: SortKey) => {
        if (ctx.get(sortKey$) === key) {
            ctx.set(sortDir$, -ctx.get(sortDir$));
        } else {
            ctx.set(sortKey$, key);
            ctx.set(sortDir$, 1);
        }
    });

    // ── async data ──
    const data$ = source<Row[] | null>(null); // null = not loaded yet
    const loading$ = derive((ctx) => ctx.get(data$) === null);

    // Fresh array on every data / sort change — the Table's rebuild trigger.
    const rows$ = derive((ctx: ReadonlyStoreCtx): Row[] => {
        const data = ctx.get(data$);
        if (data === null) return [];
        const key = ctx.get(sortKey$);
        const dir = ctx.get(sortDir$);
        return [...data].sort((a, b) => {
            const av = a[key];
            const bv = b[key];
            return dir * (av > bv ? 1 : av < bv ? -1 : 0);
        });
    });

    // Fetch once. The async body captures the ctx (§4.4); `alive` guards
    // the write if the subtree is torn down before the delay elapses
    // (flipped by `beforeDestroy$` in the view below).
    let alive = true;
    const load$ = mutate((ctx) => {
        if (ctx.get(data$) !== null || !alive) return;
        (async () => {
            const rows = await getFakeData();
            if (alive) ctx.set(data$, rows);
        })();
    });
    const abort$ = mutate(() => {
        alive = false;
    });

    return { rows$, loading$, load$, abort$, sortKey$, sortDir$, sortBy$ };
}

// ── cell factories ────────────────────────────────────────────────────────

/** Clickable header cell: the active column is highlighted and shows a
 *  direction marker — reactive *content* inside a once-built header cell,
 *  driven by `Val` props (`derive` closures re-read on each layout pass). */
function HeaderCell(st: TableState, key: SortKey, label: string): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx) => ctx.set(st.sortBy$, key)),
            child: Container({
                padding: 8,
                queryKey: [`hdr-${key}`],
                color: derive((ctx) =>
                    ctx.get(st.sortKey$) === key
                        ? Color.hex("#334155")
                        : Color.hex("#0f172a"),
                ),
                children: [
                    Text({
                        // ASCII markers — the bundled canvas font lacks the
                        // ▲/▼ glyphs (they render as tofu boxes).
                        text: derive((ctx) => {
                            if (ctx.get(st.sortKey$) !== key) return label;
                            return `${label} ${ctx.get(st.sortDir$) > 0 ? "^" : "v"}`;
                        }),
                        fontSize: 12,
                        color: Color.hex("#94a3b8"),
                    }),
                ],
            }),
        }),
    });
}

/** Body cell: padded text container at the Table's tight column width. */
function Cell(text: string, emphasize = false): Element {
    return Container({
        padding: 8,
        children: [
            Text({
                text,
                fontSize: 14,
                color: emphasize ? Color.hex("#f8fafc") : Color.hex("#cbd5e1"),
            }),
        ],
    });
}

// ── view ──────────────────────────────────────────────────────────────────

const App = view(() => {
    const st = createTableState();

    // `lifecycleView` ties the fetch to the subtree's mount: `load$` fires
    // once when the table mounts, `abort$` stops a pending fetch from
    // writing into a torn-down store if the case is switched mid-delay.
    return Expanded({
        child: lifecycleView(() => ({
            element: Container({
                // Dark base — see the table-basic case for why.
                color: Color.hex("#0b1220"),
                children: [
                    Column({
                        crossAlignment: CrossAxisAlignment.Stretch,
                        children: [
                            Table({
                                queryKey: ["table-reactive"],
                                columns: [
                                    { flex: 2 },
                                    { flex: 1 },
                                    { flex: 1 },
                                ],
                                rows: st.rows$,
                                headerExtent: 34,
                                rowExtent: 34,
                                stripeColor: Color.hex("#1e293b"),
                                dividerColor: Color.hex("#334155"),
                                dividerThickness: 1,
                                buildHeader: () =>
                                    COLS.map((c) =>
                                        HeaderCell(st, c.key, c.label),
                                    ),
                                build: (row) => [
                                    Cell(row.name, true),
                                    Cell(String(row.moons)),
                                    Cell(row.gravity.toFixed(1)),
                                ],
                            }),
                            SizedBox({ height: 12 }),
                            Text({
                                text: derive((ctx) =>
                                    ctx.get(st.loading$)
                                        ? "Loading…"
                                        : `Loaded ${ctx.get(st.rows$).length} rows · click a header to sort`,
                                ),
                                fontSize: 12,
                                color: Color.hex("#64748b"),
                            }),
                        ],
                    }),
                ],
            }),
            onMounted$: st.load$,
            beforeDestroy$: st.abort$,
        })),
    });
});

export function start() {
    mount(App);
}
