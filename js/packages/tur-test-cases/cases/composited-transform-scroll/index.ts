import {
    Column,
    CompositedTransformFollower,
    CompositedTransformTarget,
    Container,
    CrossAxisAlignment,
    createLayerLink,
    mount,
    Positioned,
    ScrollView,
    SizedBox,
    Stack,
    view,
} from "tur:std";

// Target lives inside a scrollable Column (at content offset y=100). The
// follower is at the root overlay. Scrolling shifts the target's absolute
// position; the follower must track it.
const App = view(() => {
    const link = createLayerLink();
    return Stack()
        .children([
            SizedBox().width(400).height(600).build(),
            Positioned()
                .left(0)
                .top(0)
                .width(200)
                .height(200)
                .child(
                    ScrollView()
                        .queryKey(["sv"])
                        .child(
                            Column()
                                .crossAlignment(CrossAxisAlignment.Start)
                                .children([
                                    SizedBox().width(50).height(100).build(),
                                    CompositedTransformTarget({ link })
                                        .child(
                                            SizedBox()
                                                .width(40)
                                                .height(40)
                                                .build(),
                                        )
                                        .build(),
                                    SizedBox().width(50).height(200).build(),
                                ])
                                .build(),
                        )
                        .build(),
                )
                .build(),
            CompositedTransformFollower({ link })
                .child(Container().width(15).height(15).color("red").build())
                .build(),
        ])
        .build();
});

export function start() {
    mount(App);
}
