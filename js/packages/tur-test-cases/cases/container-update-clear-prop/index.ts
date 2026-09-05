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

const green = Color.rgba(34, 197, 94, 255);
const gray = Color.rgba(226, 232, 240, 255);

const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so these atoms
    // are stable for the life of the tree — no need to hoist them to module
    // level.
    const checked$ = source(true);
    const color$ = derive((ctx) => (ctx.get(checked$) ? green : undefined));
    const borderColor$ = derive((ctx) => (ctx.get(checked$) ? green : gray));

    return Container({
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
    });
});

export function start() {
    mount(App);
}
