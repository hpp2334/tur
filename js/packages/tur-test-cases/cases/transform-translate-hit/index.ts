import {
    Condition,
    Container,
    derive,
    type Mutation,
    mount,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    SizedBox,
    Stack,
    source,
    Transform,
    view,
} from "tur:std";

// A clickable box is laid out at (0,0) but painted at (100,80) via `Transform`
// `translateX`/`translateY` (a paint-only translate). Its painted center is
// (120,100). Clicking there must register — which requires hit-testing to
// account for the transform. Before the `relative_transform` hit-test fix,
// hit-testing ignored the paint transform, so the click missed.
const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so this atom is
    // stable for the life of the tree — no need to hoist it to module level.
    const hit$ = source(false);

    return Stack({
        children: [
            // Sizes the root Stack to the full canvas so the box's absolute
            // position is deterministic (otherwise the Stack shrinks to its
            // 40×40 content and gets centered).
            SizedBox({ width: 400, height: 600 }),
            Transform({
                translateX: 100,
                translateY: 80,
                child: PointerInteract({
                    onClick: mutate((ctx) =>
                        ctx.set(hit$, true),
                    ) as unknown as Mutation<[PointerInteractEvent], void>,
                    child: Container({
                        width: 40,
                        height: 40,
                        color: "#4f46e5",
                    }),
                }),
            }),
            // A second box appears once the click landed — observable from the
            // element tree so the test can assert the hit-test found the box.
            Condition({
                condition: derive((ctx) => ctx.get(hit$)),
                child: () =>
                    Container({ width: 10, height: 10, color: "#dc2626" }),
            }),
        ],
    });
});

export function start() {
    mount(App);
}
