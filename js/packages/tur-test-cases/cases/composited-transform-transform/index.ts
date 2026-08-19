import {
    CompositedTransformFollower,
    CompositedTransformTarget,
    Container,
    createLayerLink,
    createStore,
    Positioned,
    SizedBox,
    Stack,
    Transform,
    view,
} from "tur:std";

export const store = createStore();

// Target is wrapped in a `Transform` (paint-only translate). Layout places the
// Transform at (20, 20); the translate (50, 10) is applied at paint only. The
// follower must compose the target's full world affine and land at
// (20 + 50, 20 + 10) = (70, 30) — not the layout position (20, 20).
export default view(() => {
    const link = createLayerLink();
    return Stack({
        children: [
            SizedBox({ width: 400, height: 600 }),
            Positioned({
                left: 20,
                top: 20,
                child: Transform({
                    translateX: 50,
                    translateY: 10,
                    child: CompositedTransformTarget({
                        link,
                        child: SizedBox({ width: 40, height: 40 }),
                    }),
                }),
            }),
            CompositedTransformFollower({
                link,
                child: Container({ width: 15, height: 15, color: "red" }),
            }),
        ],
    });
});
