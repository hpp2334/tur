import {
    Column,
    CrossAxisAlignment,
    derive,
    mutate,
    PointerInteract,
    source,
    Text,
    view,
} from "tur:std";

const content$ = source("before");

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            PointerInteract({
                onClick: mutate((ctx, _ev) => ctx.set(content$, "after")),
                child: Text({
                    text: derive((ctx) => ctx.get(content$)),
                    queryKey: ["click-text"],
                }),
            }),
        ],
    }),
);
