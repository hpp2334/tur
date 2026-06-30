import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    view,
    derive,
    MouseRegion,
    mutate,
    source,
    Text,
} from "@tur/edgy";

const state$ = source("idle");

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            MouseRegion({
                onEnter: mutate(({ set }, _ev) => set(state$, "entered")),
                onExit: mutate(({ set }, _ev) => set(state$, "exited")),
                child: Container({
                    width: 100,
                    height: 50,
                    color: Color.hex("#cccccc"),
                    alignment: Alignment.Center,
                    children: [
                        Text({
                            text: derive((g) => g(state$)),
                            queryKey: ["region-text"],
                        }),
                    ],
                }),
            }),
        ],
    }),
);
