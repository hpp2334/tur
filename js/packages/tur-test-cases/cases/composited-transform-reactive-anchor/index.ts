import {
    Alignment,
    CompositedTransformFollower,
    CompositedTransformTarget,
    Container,
    createLayerLink,
    createStore,
    derive,
    type Mutation,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    Positioned,
    SizedBox,
    Stack,
    source,
    view,
} from "tur:std";
export const store = createStore();

// Reactive anchor: `targetAnchor` is driven by a source. A button flips it
// from TopLeft to BottomRight; the follower must relocate to the target's
// bottom-right corner on the next frame.
const anchor$ = source(Alignment.TopLeft);

export default view(() => {
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
                targetAnchor: derive((ctx) => ctx.get(anchor$)),
                child: Container({ width: 20, height: 20, color: "red" }),
            }),
            // Button at (20, 540), 60×30 — click flips the anchor.
            Positioned({
                left: 20,
                top: 540,
                child: PointerInteract({
                    onClick: mutate((ctx) =>
                        ctx.set(anchor$, Alignment.BottomRight),
                    ) as unknown as Mutation<[PointerInteractEvent], void>,
                    child: Container({
                        width: 60,
                        height: 30,
                        color: "#4f46e5",
                    }),
                }),
            }),
        ],
    });
});
