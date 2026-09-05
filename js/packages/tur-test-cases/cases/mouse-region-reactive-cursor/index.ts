import {
    Color,
    Column,
    Container,
    type Cursor,
    derive,
    MouseRegion,
    mount,
    type Store,
    source,
    view,
} from "tur:std";

// A reactive cursor driven by a source, so Rust tests can flip it via
// `globalThis.__setCursor` and assert the host cursor updates after a flush.
// Module-level on purpose: the test seam in `start` below needs this handle
// (shared across the view fn and `start`), not because views re-run.
const cursor$ = source<Cursor>("pointer");

const App = view(() =>
    Column({
        children: [
            MouseRegion({
                cursor: derive((ctx) => ctx.get(cursor$)),
                child: Container({
                    width: 100,
                    height: 50,
                    color: Color.hex("#cccccc"),
                    children: [],
                }),
            }),
        ],
    }),
);

export function start({ store }: { store: Store }) {
    Object.assign(globalThis, {
        __setCursor: (c: Cursor): void => {
            store.set(cursor$, c);
        },
    });
    mount(App);
}
