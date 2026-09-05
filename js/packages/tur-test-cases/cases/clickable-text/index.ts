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

const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so this atom is
    // stable for the life of the tree — no need to hoist it to module level.
    const content$ = source("before");

    return Column({
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
    });
});

export function start() {
    mount(App);
}
