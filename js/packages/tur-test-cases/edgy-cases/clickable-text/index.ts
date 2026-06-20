import {
    Column,
    CrossAxisAlignment,
    component,
    derive,
    mutate,
    PointerInteract,
    source,
    Text,
} from "@tur/edgy";

const content$ = source("before");

export default component(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            PointerInteract({
                onClick: mutate(({ set }, _ev) => set(content$, "after")),
                child: Text({
                    text: derive((g) => g(content$)),
                    queryKey: ["click-text"],
                }),
            }),
        ],
    }),
);
