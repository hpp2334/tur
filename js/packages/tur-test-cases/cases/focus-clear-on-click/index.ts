import {
    Column,
    createTextEditingController,
    derive,
    Input,
    mutate,
    PointerInteract,
    source,
    Text,
    view,
} from "tur:std";

// Regression fixture for the github-viewer crash: a focused `Input` (editable,
// focusable) above a non-focusable `PointerInteract` button. Clicking the
// button while the input is focused exercises the gesture handler's
// "clear focus when pointer-up lands outside any focusable" path, which used
// to panic with "RefCell already borrowed" because a `let`-chain kept an
// immutable `focus_manager` borrow alive across the `borrow_mut()`.
const controller = createTextEditingController({});
const clicks$ = source(0);

export default view(() =>
    Column({
        children: [
            Input({
                controller,
                fontSize: 14,
                width: 200,
                height: 30,
                queryKey: ["editable"],
            }),
            PointerInteract({
                onClick: mutate((ctx) =>
                    ctx.set(clicks$, ctx.get(clicks$) + 1),
                ),
                child: Text({
                    text: derive((ctx) => `clicks: ${ctx.get(clicks$)}`),
                    queryKey: ["button"],
                }),
            }),
        ],
    }),
);
