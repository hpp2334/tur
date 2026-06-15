import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    mutate,
    PointerInteract,
    render,
    source,
    Text,
} from "@tur/edgy";

const state$ = source("idle");

render(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            PointerInteract({
                onPointerEnter: mutate(({ set }) => set(state$, "entered")),
                onPointerExit: mutate(({ set }) => set(state$, "exited")),
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
