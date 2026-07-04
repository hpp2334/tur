import {
    Alignment,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    mutate,
    PointerInteract,
    source,
    Text,
    view,
} from "builtin:tur/core";
import { COLORS } from "./utils";

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
                        onClick: mutate(({ get, set }, _ev) =>
                            set(count$, get(count$) + 1),
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
