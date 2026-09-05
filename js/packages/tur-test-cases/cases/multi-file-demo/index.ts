import {
    Alignment,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    mount,
    mutate,
    PointerInteract,
    source,
    Text,
    view,
} from "tur:std";
import { COLORS } from "./utils";

const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so this atom is
    // stable for the life of the tree (the multi-file split is about code
    // organization — shared *values* like COLORS — not about state placement).
    const count$ = source(0);

    return Container({
        color: COLORS.bg,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    Text({
                        text: derive(
                            (ctx) => `Multi-file count: ${ctx.get(count$)}`,
                        ),
                        fontSize: 24,
                        color: COLORS.text,
                    }),
                    PointerInteract({
                        onClick: mutate((ctx, _ev) =>
                            ctx.set(count$, ctx.get(count$) + 1),
                        ),
                        child: Container({
                            width: 120,
                            height: 44,
                            borderRadius: 8,
                            color: COLORS.primary,
                            alignment: Alignment.Center,
                            children: [
                                Text({
                                    text: "+1",
                                    fontSize: 18,
                                    color: COLORS.white,
                                }),
                            ],
                        }),
                    }),
                ],
            }),
        ],
    });
});

export function start() {
    mount(App);
}
