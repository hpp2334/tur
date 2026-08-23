import {
    Color,
    Column,
    Container,
    MouseRegion,
    mount,
    mutate,
    type Store,
    source,
    view,
} from "tur:std";

// A single shared hover source, mirroring the playground sidebar pattern.
// Each region sets it on enter and clears it (unconditionally) on exit.
const hover$ = source("");

const App = view(() =>
    Column({
        children: [
            MouseRegion({
                onEnter: mutate((ctx, _ev) => ctx.set(hover$, "A")),
                onExit: mutate((ctx, _ev) => ctx.set(hover$, "")),
                child: Container({
                    width: 100,
                    height: 50,
                    color: Color.hex("#ef4444"),
                    queryKey: ["a"],
                }),
            }),
            MouseRegion({
                onEnter: mutate((ctx, _ev) => ctx.set(hover$, "B")),
                onExit: mutate((ctx, _ev) => ctx.set(hover$, "")),
                child: Container({
                    width: 100,
                    height: 50,
                    color: Color.hex("#22c55e"),
                    queryKey: ["b"],
                }),
            }),
        ],
    }),
);

export function start({ store }: { store: Store }) {
    Object.assign(globalThis, {
        __getHover: (): string => store.get(hover$),
    });
    mount(App);
}
