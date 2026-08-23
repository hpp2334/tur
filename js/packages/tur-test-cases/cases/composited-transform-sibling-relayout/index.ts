import {
    CompositedTransformFollower,
    CompositedTransformTarget,
    Container,
    createLayerLink,
    derive,
    type Mutation,
    mount,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    Positioned,
    SizedBox,
    Stack,
    source,
    view,
} from "tur:std";

// Reproduces the follower "flash to top-left" bug. The follower is a
// non-positioned child of the root Stack, so layout assigns it offset (0,0);
// the CompositedTransformSubsystem then moves it to the target (100, 80).
// Flipping a reactive SIBLING's height forces the Stack to relayout, which
// re-assigns the follower's offset back to (0,0). If the subsystem does not
// re-correct within the same frame, the follower paints at (0,0) for one
// frame before snapping back — the visible flash.
const tall$ = source(false);

const App = view(() => {
    const link = createLayerLink();
    return Stack({
        children: [
            SizedBox({ width: 400, height: 600 }),
            Positioned({
                left: 100,
                top: 80,
                child: CompositedTransformTarget({
                    link,
                    child: SizedBox({ width: 60, height: 40 }),
                }),
            }),
            CompositedTransformFollower({
                link,
                child: Container({ width: 20, height: 20, color: "#dc2626" }),
            }),
            // Reactive sibling (non-positioned): height flips 10 ↔ 120 so the
            // Stack's max child height changes, forcing a genuine relayout.
            Container({
                width: 40,
                height: derive((ctx) => (ctx.get(tall$) ? 120 : 10)),
                color: "#4f46e5",
            }),
            // Button (Positioned, away from the follower/target) to flip tall$.
            Positioned({
                left: 300,
                top: 540,
                child: PointerInteract({
                    onClick: mutate((ctx) =>
                        ctx.set(tall$, true),
                    ) as unknown as Mutation<[PointerInteractEvent], void>,
                    child: Container({
                        width: 60,
                        height: 30,
                        color: "#16a34a",
                    }),
                }),
            }),
        ],
    });
});

export function start() {
    mount(App);
}
