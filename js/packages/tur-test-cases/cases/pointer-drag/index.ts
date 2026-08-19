import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    createStore,
    derive,
    mutate,
    PointerInteract,
    source,
    Text,
    view,
} from "tur:std";
export const store = createStore();

const lastX$ = source(0);
const lastY$ = source(0);
const phase$ = source("idle");

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            PointerInteract({
                onPointerDown: mutate((ctx, ev) => {
                    ctx.set(lastX$, Math.round(ev.global.x));
                    ctx.set(lastY$, Math.round(ev.global.y));
                    ctx.set(phase$, "down");
                }),
                onPointerMove: mutate((ctx, ev) => {
                    ctx.set(lastX$, Math.round(ev.global.x));
                    ctx.set(lastY$, Math.round(ev.global.y));
                    ctx.set(phase$, "move");
                }),
                onPointerUp: mutate((ctx, ev) => {
                    ctx.set(lastX$, Math.round(ev.global.x));
                    ctx.set(lastY$, Math.round(ev.global.y));
                    ctx.set(phase$, "up");
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
