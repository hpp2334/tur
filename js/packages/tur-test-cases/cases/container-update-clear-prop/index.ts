import {
    BorderPosition,
    type Brush,
    Color,
    Container,
    derive,
    mount,
    mutate,
    PointerInteract,
    source,
    view,
} from "tur:std";

const checked$ = source(true);
const green = Color.rgba(34, 197, 94, 255);
const gray = Color.rgba(226, 232, 240, 255);
const color$ = derive((ctx) => (ctx.get(checked$) ? green : undefined));
const borderColor$ = derive((ctx) => (ctx.get(checked$) ? green : gray));

const App = view(() =>
    Container({
        height: 100,
        width: 200,
        padding: 20,
        children: [
            PointerInteract({
                onClick: mutate((ctx, _ev) => ctx.set(checked$, false)),
                child: Container({
                    width: 40,
                    height: 40,
                    borderRadius: 8,
                    color: color$ as unknown as Brush,
                    borderWidth: 2,
                    borderColor: borderColor$,
                    borderPosition: BorderPosition.Center,
                }),
            }),
        ],
    }),
);

export function start() {
    mount(App);
}
