import {
    Alignment,
    CompositedTransformFollower,
    CompositedTransformTarget,
    createLayerLink,
    Positioned,
    SizedBox,
    Stack,
    view,
} from "tur:std";
import { Container } from "tur:std";

// Regression: `followerAnchor` must be honored. Target (blue, 60×40) sits at
// (100, 80) → its bottom-right is (160, 120). The follower (red, 60×40) with
// targetAnchor BottomRight + followerAnchor TopRight must place its top-right
// at (160, 120), i.e. its top-left at (100, 120) — red box directly below the
// blue box, right edges flush. If followerAnchor is dropped (treated as
// TopLeft) the follower's top-left lands at (160, 120) and the red box
// overhangs off-screen right.
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
            Positioned({
                left: 100,
                top: 80,
                child: Container({ width: 60, height: 40, color: "#2563eb" }),
            }),
            CompositedTransformFollower({
                link,
                targetAnchor: Alignment.BottomRight,
                followerAnchor: Alignment.TopRight,
                child: Container({ width: 60, height: 40, color: "#dc2626" }),
            }),
        ],
    });
});
