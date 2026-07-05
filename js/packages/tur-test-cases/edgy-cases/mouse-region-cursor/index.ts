import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    MouseRegion,
    mutate,
    source,
    Text,
    view,
} from "builtin:tur/std";

const state$ = source("idle");

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            MouseRegion({
                cursor: "col-resize",
                onEnter: mutate(({ set }, _ev) => set(state$, "entered")),
                onExit: mutate(({ set }, _ev) => set(state$, "exited")),
                child: Container({
                    width: 100,
                    height: 50,
                    color: Color.hex("#cccccc"),
                    alignment: Alignment.Center,
                    children: [
                        Text({
                            text: derive((ctx) => ctx.get(state$)),
                            queryKey: ["mr-state"],
                        }),
                    ],
                }),
            }),
        ],
    }),
);
