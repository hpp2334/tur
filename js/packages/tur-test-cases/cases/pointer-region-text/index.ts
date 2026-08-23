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

const state$ = source("idle");

const App = view(() =>
    Column({
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
    }),
);

export function start() {
    mount(App);
}
