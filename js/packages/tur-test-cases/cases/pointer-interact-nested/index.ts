import {
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    HitTestBehavior,
    MainAxisSize,
    mutate,
    PointerInteract,
    Row,
    source,
    Text,
    view,
} from "tur:std";

const outerClicks$ = source(0);
const innerClicks$ = source(0);
const translucentOuterClicks$ = source(0);
const translucentInnerClicks$ = source(0);

export default view(() =>
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
                    (ctx) =>
                        `opaque:${ctx.get(outerClicks$)}/${ctx.get(innerClicks$)}`,
                ),
                queryKey: ["result-opaque"],
            }),
            Text({
                text: derive(
                    (ctx) =>
                        `translucent:${ctx.get(translucentOuterClicks$)}/${ctx.get(translucentInnerClicks$)}`,
                ),
                queryKey: ["result-translucent"],
            }),
        ],
    }),
);
