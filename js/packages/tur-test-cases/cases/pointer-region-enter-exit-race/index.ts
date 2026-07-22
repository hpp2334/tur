import {
    Color,
    Column,
    Container,
    get,
    MouseRegion,
    mutate,
    set,
    source,
    view,
} from "tur:std";

// A single shared hover source, mirroring the playground sidebar pattern.
// Each region sets it on enter and clears it (unconditionally) on exit.
const hover$ = source("");

Object.assign(globalThis, {
    __getHover: (): string => get(hover$),
});

export default view(() =>
    Column({
        children: [
            MouseRegion({
                onEnter: mutate((_ctx, _ev) => set(hover$, "A")),
                onExit: mutate((_ctx, _ev) => set(hover$, "")),
                child: Container({
                    width: 100,
                    height: 50,
                    color: Color.hex("#ef4444"),
                    queryKey: ["a"],
                }),
            }),
            MouseRegion({
                onEnter: mutate((_ctx, _ev) => set(hover$, "B")),
                onExit: mutate((_ctx, _ev) => set(hover$, "")),
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
