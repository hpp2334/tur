import {
    Alignment,
    Column,
    Container,
    CrossAxisAlignment,
    component,
    derive,
    mutate,
    PointerInteract,
    source,
    Text,
} from "@tur/edgy";
import { COLORS } from "./utils";

const count$ = source(0);

export default component(() =>
    Container({
        color: COLORS.bg,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    Text({
                        text: derive((g) => `Multi-file count: ${g(count$)}`),
                        fontSize: 24,
                        color: COLORS.text,
                    }),
                    PointerInteract({
                        onClick: mutate(({ get, set }) =>
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
