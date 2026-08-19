import {
    Color,
    Column,
    Container,
    type Cursor,
    createStore,
    derive,
    MouseRegion,
    source,
    view,
} from "tur:std";
export const store = createStore();

// A reactive cursor driven by a source, so Rust tests can flip it via
// `globalThis.__setCursor` and assert the host cursor updates after a flush.
const cursor$ = source<Cursor>("pointer");

Object.assign(globalThis, {
    __setCursor: (c: Cursor): void => {
        store.set(cursor$, c);
    },
});

export default view(() =>
    Column({
        children: [
            MouseRegion({
                cursor: derive(() => store.get(cursor$)),
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
