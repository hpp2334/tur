import {
    CompositedTransformFollower,
    CompositedTransformTarget,
    Container,
    createLayerLink,
    Positioned,
    SizedBox,
    Stack,
    Transform,
    view,
} from "tur:std";

// Regression for affine-based follower tracking. The follower is nested inside
// a `Transform` with a PAINT-ONLY translate (translateX: 50, translateY: 30),
// so its `computed_layout.offset` stays (0,0) — the translate lives only in the
// Transform's `relative_transform`.
//
// The follower must still track the target's top-left in WORLD space (100, 80).
// With offset-subtraction tracking (the old approach), the subsystem ignored
// the ancestor Transform and placed the follower at (150, 110) — the ancestor
// translate stacked on top of the desired point. With affine-inverse tracking
// the subsystem solves `parent_world⁻¹ · translate(desired)` and the follower
// lands exactly at (100, 80).
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
            Transform({
                translateX: 50,
                translateY: 30,
                child: CompositedTransformFollower({
                    link,
                    child: Container({
                        width: 20,
                        height: 20,
                        color: "#dc2626",
                    }),
                }),
            }),
        ],
    });
});
