import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    MouseRegion,
    mount,
    mutate,
    source,
    Text,
    view,
} from "tur:std";

const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so this atom is
    // stable for the life of the tree — no need to hoist it to module level.
    const state$ = source("idle");

    return Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            MouseRegion({
                onEnter: mutate((ctx, _ev) => ctx.set(state$, "entered")),
                onExit: mutate((ctx, _ev) => ctx.set(state$, "exited")),
                child: Container({
                    width: 100,
                    height: 50,
                    color: Color.hex("#cccccc"),
                    alignment: Alignment.Center,
                    children: [
                        Text({
                            text: derive((ctx) => ctx.get(state$)),
                            queryKey: ["region-text"],
                        }),
                    ],
                }),
            }),
        ],
    });
});

export function start() {
    mount(App);
}
