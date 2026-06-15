import {
    Column,
    CrossAxisAlignment,
    derive,
    mutate,
    PointerInteract,
    render,
    source,
    Text,
} from "@tur/edgy";

const content$ = source("before");

render(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            PointerInteract({
                onClick: mutate(({ set }) => set(content$, "after")),
                child: Text({
                    text: derive((g) => g(content$)),
                    queryKey: ["click-text"],
                }),
            }),
        ],
    }),
);
