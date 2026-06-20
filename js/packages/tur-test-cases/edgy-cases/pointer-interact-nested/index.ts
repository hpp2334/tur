import {
    Column,
    Container,
    CrossAxisAlignment,
    component,
    derive,
    HitTestBehavior,
    MainAxisSize,
    mutate,
    PointerInteract,
    Row,
    source,
    Text,
} from "@tur/edgy";

const outerClicks$ = source(0);
const innerClicks$ = source(0);
const translucentOuterClicks$ = source(0);
const translucentInnerClicks$ = source(0);

export default component(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        mainAxisSize: MainAxisSize.Min,
        children: [
            PointerInteract({
                onClick: mutate(({ get, set }, _ev) =>
                    set(outerClicks$, get(outerClicks$) + 1),
                ),
                child: Container({
                    queryKey: ["outer-opaque"],
                    width: 80,
                    height: 40,
                    children: [
                        Row({
                            children: [
                                PointerInteract({
                                    onClick: mutate(({ get, set }, _ev) =>
                                        set(
                                            innerClicks$,
                                            get(innerClicks$) + 1,
                                        ),
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
                onClick: mutate(({ get, set }, _ev) =>
                    set(
                        translucentOuterClicks$,
                        get(translucentOuterClicks$) + 1,
                    ),
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
                                    onClick: mutate(({ get, set }, _ev) =>
                                        set(
                                            translucentInnerClicks$,
                                            get(translucentInnerClicks$) + 1,
                                        ),
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
                text: derive(
                    (g) => `opaque:${g(outerClicks$)}/${g(innerClicks$)}`,
                ),
                queryKey: ["result-opaque"],
            }),
            Text({
                text: derive(
                    (g) =>
                        `translucent:${g(translucentOuterClicks$)}/${g(translucentInnerClicks$)}`,
                ),
                queryKey: ["result-translucent"],
            }),
        ],
    }),
);
