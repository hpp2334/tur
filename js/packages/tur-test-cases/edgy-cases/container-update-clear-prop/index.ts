import {
    BorderPosition,
    Color,
    Container,
    derive,
    mutate,
    PointerInteract,
    source,
    view,
} from "@tur/edgy";

const checked$ = source(true);
const green = Color.rgba(34, 197, 94, 255);
const gray = Color.rgba(226, 232, 240, 255);
const color$ = derive((g) => (g(checked$) ? green : undefined));
const borderColor$ = derive((g) => (g(checked$) ? green : gray));

export default view(() =>
    Container({
        height: 100,
        width: 200,
        padding: 20,
        children: [
            PointerInteract({
                onClick: mutate(({ set }, _ev) => set(checked$, false)),
                child: Container({
                    width: 40,
                    height: 40,
                    borderRadius: 8,
                    color: color$,
                    borderWidth: 2,
                    borderColor: borderColor$,
                    borderPosition: BorderPosition.Center,
                }),
            }),
        ],
    }),
);
