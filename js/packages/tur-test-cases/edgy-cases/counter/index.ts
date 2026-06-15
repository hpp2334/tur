import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    Expanded,
    MainAxisAlignment,
    PointerInteract,
    Row,
    Text,
    derive,
    mutate,
    render,
    source,
} from "@tur/edgy";

const count$ = source(0);

render(() =>
    Expanded({
        child: Container({
            color: Color.hex("#f8fafc"),
            children: [
                Column({
                    mainAlignment: MainAxisAlignment.Center,
                    crossAlignment: CrossAxisAlignment.Center,
                    children: [
                        Text({
                            text: derive((g) => `Count: ${g(count$)}`),
                            queryKey: ["count"],
                            fontSize: 36,
                            color: Color.hex("#1e293b"),
                        }),
                        Row({
                            mainAlignment: MainAxisAlignment.Center,
                            children: [
                                PointerInteract({
                                    onClick: mutate(({ get, set }) =>
                                        set(count$, get(count$) + 1),
                                    ),
                                    child: Container({
                                        width: 100,
                                        height: 44,
                                        borderRadius: 8,
                                        color: Color.hex("#6366f1"),
                                        alignment: Alignment.Center,
                                        children: [
                                            Text({
                                                text: "+1",
                                                fontSize: 18,
                                                color: Color.hex("#ffffff"),
                                            }),
                                        ],
                                    }),
                                }),
                            ],
                        }),
                    ],
                }),
            ],
        }),
    }),
);
