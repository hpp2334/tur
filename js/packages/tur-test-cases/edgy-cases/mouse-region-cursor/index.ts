import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    component,
    MouseRegion,
    Text,
    derive,
    source,
    mutate,
} from "@tur/edgy";

const state$ = source("idle");

export default component(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            MouseRegion({
                cursor: "col-resize",
                onEnter: mutate(({ set }) => set(state$, "entered")),
                onExit: mutate(({ set }) => set(state$, "exited")),
                child: Container({
                    width: 100,
                    height: 50,
                    color: Color.hex("#cccccc"),
                    alignment: Alignment.Center,
                    children: [
                        Text({
                            text: derive((g) => g(state$)),
                            queryKey: ["mr-state"],
                        }),
                    ],
                }),
            }),
        ],
    }),
);
