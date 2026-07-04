import {
    Alignment,
    Color,
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

const lastX$ = source(0);
const lastY$ = source(0);
const phase$ = source("idle");

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            PointerInteract({
                onPointerDown: mutate(({ set }, ev) => {
                    set(lastX$, Math.round(ev.global.x));
                    set(lastY$, Math.round(ev.global.y));
                    set(phase$, "down");
                }),
                onPointerMove: mutate(({ set }, ev) => {
                    set(lastX$, Math.round(ev.global.x));
                    set(lastY$, Math.round(ev.global.y));
                    set(phase$, "move");
                }),
                onPointerUp: mutate(({ set }, ev) => {
                    set(lastX$, Math.round(ev.global.x));
                    set(lastY$, Math.round(ev.global.y));
                    set(phase$, "up");
                }),
                child: Container({
                    width: 100,
                    height: 50,
                    color: Color.hex("#cccccc"),
                    alignment: Alignment.Center,
                    children: [
                        Text({
                            text: derive((ctx) => ctx.get(phase$)),
                            queryKey: ["drag-phase"],
                        }),
                    ],
                }),
            }),
            Text({
                text: derive((ctx) => `${ctx.get(lastX$)},${ctx.get(lastY$)}`),
                queryKey: ["drag-pos"],
            }),
        ],
    }),
);
