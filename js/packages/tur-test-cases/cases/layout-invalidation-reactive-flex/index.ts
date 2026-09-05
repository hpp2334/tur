import {
    Color,
    Container,
    derive,
    Expanded,
    mount,
    Row,
    type Store,
    source,
    view,
} from "tur:std";

// Two reactive flex weights. Exposed to Rust tests via globalThis so the test
// can flip them without a click (a click would mark extra nodes dirty via the
// gesture handler and mask the layout-invalidation bug).
// Module-level on purpose: the test seam in `start` below needs these handles
// (shared across the view fn and `start`), not because views re-run.
const flexA$ = source(1);
const flexB$ = source(1);

const App = view(() =>
    Row({
        children: [
            Expanded({
                flex: derive((ctx) => ctx.get(flexA$)),
                child: Container({
                    color: Color.hex("#ef4444"),
                    queryKey: ["a"],
                }),
            }),
            Expanded({
                flex: derive((ctx) => ctx.get(flexB$)),
                child: Container({
                    color: Color.hex("#22c55e"),
                    queryKey: ["b"],
                }),
            }),
        ],
    }),
);

export function start({ store }: { store: Store }) {
    Object.assign(globalThis, {
        __setFlex: (a: number, b: number): void => {
            store.set(flexA$, a);
            store.set(flexB$, b);
        },
    });
    mount(App);
}
