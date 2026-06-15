import {
    Column,
    Container,
    CrossAxisAlignment,
    HitTestBehavior,
    MainAxisSize,
    PointerInteract,
    Row,
    Text,
    derive,
    mutate,
    render,
    source,
} from "@tur/edgy";

const outerClicks$ = source(0);
const innerClicks$ = source(0);
const translucentOuterClicks$ = source(0);
const translucentInnerClicks$ = source(0);

render(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        mainAxisSize: MainAxisSize.Min,
        children: [
            PointerInteract({
                onClick: mutate(({ get, set }) => set(outerClicks$, get(outerClicks$) + 1)),
                child: Container({
                    queryKey: ["outer-opaque"],
                    width: 80,
                    height: 40,
                    children: [
                        Row({
                            children: [
                                PointerInteract({
                                    onClick: mutate(({ get, set }) =>
                                        set(innerClicks$, get(innerClicks$) + 1),
                                    ),
                                    child: Container({
                                        queryKey: ["inner-opaque"],
                                        width: 60,
                                        height: 30,
                                    }),
                                }),
                            ],
                        }),
                    ],
                }),
            }),
            PointerInteract({
                onClick: mutate(({ get, set }) =>
                    set(translucentOuterClicks$, get(translucentOuterClicks$) + 1),
                ),
                child: Container({
                    queryKey: ["outer-translucent"],
                    width: 80,
                    height: 40,
                    children: [
                        Row({
                            children: [
                                PointerInteract({
                                    behavior: HitTestBehavior.Translucent,
                                    onClick: mutate(({ get, set }) =>
                                        set(translucentInnerClicks$, get(translucentInnerClicks$) + 1),
                                    ),
                                    child: Container({
                                        queryKey: ["inner-translucent"],
                                        width: 60,
                                        height: 30,
                                    }),
                                }),
                            ],
                        }),
                    ],
                }),
            }),
            Text({
                text: derive((g) => `opaque:${g(outerClicks$)}/${g(innerClicks$)}`),
                queryKey: ["result-opaque"],
            }),
            Text({
                text: derive((g) => `translucent:${g(translucentOuterClicks$)}/${g(translucentInnerClicks$)}`),
                queryKey: ["result-translucent"],
            }),
        ],
    }),
);
