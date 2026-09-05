import {
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    HitTestBehavior,
    MainAxisSize,
    mount,
    mutate,
    PointerInteract,
    Row,
    source,
    Text,
    view,
} from "tur:std";

const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so these atoms
    // are stable for the life of the tree — no need to hoist them to module
    // level.
    const outerClicks$ = source(0);
    const innerClicks$ = source(0);
    const translucentOuterClicks$ = source(0);
    const translucentInnerClicks$ = source(0);

    return Column({
        crossAlignment: CrossAxisAlignment.Start,
        mainAxisSize: MainAxisSize.Min,
        children: [
            PointerInteract({
                onClick: mutate((ctx, _ev) =>
                    ctx.set(outerClicks$, ctx.get(outerClicks$) + 1),
                ),
                child: Container({
                    queryKey: ["outer-opaque"],
                    width: 80,
                    height: 40,
                    children: [
                        Row({
                            children: [
                                PointerInteract({
                                    onClick: mutate((ctx, _ev) =>
                                        ctx.set(
                                            innerClicks$,
                                            ctx.get(innerClicks$) + 1,
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
                onClick: mutate((ctx, _ev) =>
                    ctx.set(
                        translucentOuterClicks$,
                        ctx.get(translucentOuterClicks$) + 1,
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
                                    onClick: mutate((ctx, _ev) =>
                                        ctx.set(
                                            translucentInnerClicks$,
                                            ctx.get(translucentInnerClicks$) +
                                                1,
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
    });
});

export function start() {
    mount(App);
}
