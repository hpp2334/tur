import {
    Column,
    CrossAxisAlignment,
    derive,
    mount,
    mutate,
    PointerInteract,
    source,
    Text,
    view,
} from "tur:std";

const content$ = source("before");

const App = view(() =>
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

export function start() {
    mount(App);
}
