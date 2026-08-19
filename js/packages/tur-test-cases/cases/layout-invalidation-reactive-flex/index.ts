import {
    Color,
    Container,
    createStore,
    derive,
    Expanded,
    Row,
    source,
    view,
} from "tur:std";
export const store = createStore();

// Two reactive flex weights. Exposed to Rust tests via globalThis so the test
// can flip them without a click (a click would mark extra nodes dirty via the
// gesture handler and mask the layout-invalidation bug).
const flexA$ = source(1);
const flexB$ = source(1);

Object.assign(globalThis, {
    __setFlex: (a: number, b: number): void => {
        store.set(flexA$, a);
        store.set(flexB$, b);
    },
});

export default view(() =>
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
