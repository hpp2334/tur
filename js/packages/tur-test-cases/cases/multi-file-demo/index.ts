import {
    Alignment,
    Column,
    Container,
    CrossAxisAlignment,
    createStore,
    derive,
    mutate,
    PointerInteract,
    source,
    Text,
    view,
} from "tur:std";
import { COLORS } from "./utils";
export const store = createStore();

const count$ = source(0);

export default view(() =>
    Container({
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
    }),
);
