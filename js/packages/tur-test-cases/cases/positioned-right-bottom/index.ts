import { mount, Positioned, SizedBox, Stack, view } from "tur:std";

// Anchored placement against the STACK's own size. The Stack is sized by
// its non-positioned child (200x300) while the incoming constraints allow
// up to the 400x600 viewport — the anchored children must resolve against
// the Stack's final size (200x300), NOT the incoming constraints (Flutter:
// RenderStack sizes itself from non-positioned children, then constrains
// positioned children to that size and resolves their edge anchors).
const App = view(() =>
    Stack({
        children: [
            SizedBox({ width: 200, height: 300 }),
            Positioned({
                right: 8,
                bottom: 8,
                child: SizedBox({
                    width: 60,
                    height: 24,
                    queryKey: ["rb-pill"],
                }),
            }),
            Positioned({
                left: 10,
                right: 10,
                child: SizedBox({ height: 20, queryKey: ["lr-pair"] }),
            }),
        ],
    }),
);

export function start() {
    mount(App);
}
