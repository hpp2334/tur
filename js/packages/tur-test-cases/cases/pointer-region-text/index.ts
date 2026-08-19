import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    createStore,
    derive,
    MouseRegion,
    mutate,
    source,
    Text,
    view,
} from "tur:std";
export const store = createStore();

const state$ = source("idle");

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            MouseRegion({
                onEnter: mutate((_ctx, _ev) => store.set(state$, "entered")),
                onExit: mutate((_ctx, _ev) => store.set(state$, "exited")),
                child: Container({
                    width: 100,
                    height: 50,
                    color: Color.hex("#cccccc"),
                    alignment: Alignment.Center,
                    children: [
                        Text({
                            text: derive((ctx) => ctx.get(state$)),
                            queryKey: ["region-text"],
                        }),
                    ],
                }),
            }),
        ],
    }),
);
