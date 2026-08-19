import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    createStore,
    derive,
    Expanded,
    MainAxisAlignment,
    mutate,
    PointerInteract,
    Row,
    SizedBox,
    source,
    Text,
    view,
} from "tur:std";
export const store = createStore();

const count$ = source(0);

export default view(() =>
    Expanded({
        child: Container({
            color: Color.hex("#f8fafc"),
            children: [
                Column({
                    mainAlignment: MainAxisAlignment.Center,
                    crossAlignment: CrossAxisAlignment.Center,
                    children: [
                        Text({
                            text: derive((ctx) => `Count: ${ctx.get(count$)}`),
                            queryKey: ["count"],
                            fontSize: 36,
                            color: Color.hex("#1e293b"),
                        }),
                        Row({
                            mainAlignment: MainAxisAlignment.Center,
                            children: [
                                PointerInteract({
                                    onClick: mutate((ctx, _ev) =>
                                        ctx.set(count$, ctx.get(count$) - 1),
                                    ),
                                    child: Container({
                                        width: 100,
                                        height: 44,
                                        borderRadius: 8,
                                        color: Color.hex("#6366f1"),
                                        alignment: Alignment.Center,
                                        children: [
                                            Text({
                                                text: "-1",
                                                fontSize: 18,
                                                color: Color.hex("#ffffff"),
                                            }),
                                        ],
                                    }),
                                }),
                                SizedBox({ width: 12 }),
                                PointerInteract({
                                    queryKey: ["inc"],
                                    onClick: mutate((ctx, _ev) =>
                                        ctx.set(count$, ctx.get(count$) + 1),
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
