import {
    CompositedTransformFollower,
    CompositedTransformTarget,
    Container,
    createLayerLink,
    mount,
    Positioned,
    SizedBox,
    Stack,
    view,
} from "tur:std";

// Target sits at (100, 80) via a Positioned wrapper; the follower (default
// TopLeft/TopLeft anchors, zero targetOffset) should land its top-left at the
// target's top-left. The follower is a direct root-Stack child (overlay slot).
const App = view(() => {
    const link = createLayerLink();
    return Stack()
        .children([
            SizedBox().width(400).height(600).build(),
            Positioned()
                .left(100)
                .top(80)
                .child(
                    CompositedTransformTarget({ link })
                        .child(SizedBox().width(60).height(40).build())
                        .build(),
                )
                .build(),
            CompositedTransformFollower({ link })
                .child(Container().width(20).height(20).color("red").build())
                .build(),
        ])
        .build();
});

export function start() {
    mount(App);
}
