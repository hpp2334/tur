import {
    Column,
    CompositedTransformFollower,
    CompositedTransformTarget,
    Container,
    CrossAxisAlignment,
    createLayerLink,
    Positioned,
    ScrollView,
    SizedBox,
    Stack,
    view,
} from "tur:std";

// Target lives inside a scrollable Column (at content offset y=100). The
// follower is at the root overlay. Scrolling shifts the target's absolute
// position; the follower must track it.
export default view(() => {
    const link = createLayerLink();
    return Stack({
        children: [
            SizedBox({ width: 400, height: 600 }),
            Positioned({
                left: 0,
                top: 0,
                width: 200,
                height: 200,
                child: ScrollView({
                    queryKey: ["sv"],
                    child: Column({
                        crossAlignment: CrossAxisAlignment.Start,
                        children: [
                            SizedBox({ width: 50, height: 100 }),
                            CompositedTransformTarget({
                                link,
                                child: SizedBox({ width: 40, height: 40 }),
                            }),
                            SizedBox({ width: 50, height: 200 }),
                        ],
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
