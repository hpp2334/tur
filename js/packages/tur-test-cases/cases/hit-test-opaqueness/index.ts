import {
    Alignment,
    Color,
    Container,
    HitTestBehavior,
    MouseRegion,
    mount,
    mutate,
    PointerInteract,
    Positioned,
    SizedBox,
    Stack,
    view,
} from "tur:std";

// ---------------------------------------------------------------------------
// The absorption matrix (Flutter RenderBox.hitTest parity):
//   - a painted surface (Container with color) absorbs — DecoratedBox;
//   - a decoration-less Container is transparent — hits pass through;
//   - a PointerInteract (behavior: Opaque, the default) absorbs even with
//     no visible child — the invisible hit-target widget;
//   - a MouseRegion with behavior: Translucent joins the hit path without
//     absorbing — hits keep falling through to whatever is behind.
//
// Layout: the opaque gesture target (beneath) fills the 400x600 stack; a
// decoration-less full-size Container sits above it; a red 200x200 surface
// at the top-left absorbs within its rect; a translucent MouseRegion covers
// the right half (x 200..400).
// ---------------------------------------------------------------------------

let beneathDowns = 0;

Object.assign(globalThis, {
    __getBeneathDowns: (): number => beneathDowns,
});

const App = view(() =>
    Stack()
        .alignment(Alignment.TopLeft)
        .children([
            PointerInteract()
                .onPointerDown(
                    mutate(() => {
                        beneathDowns += 1;
                    }),
                )
                .child(Container().width(400).height(600).build())
                .build(),
            Container().width(400).height(600).queryKey(["plain"]).build(),
            Container()
                .width(200)
                .height(200)
                .color(Color.hex("#ef4444"))
                .queryKey(["red"])
                .build(),
            Positioned()
                .left(200)
                .top(0)
                .width(200)
                .height(600)
                .child(
                    MouseRegion()
                        .behavior(HitTestBehavior.Translucent)
                        .child(
                            SizedBox()
                                .width(200)
                                .height(600)
                                .queryKey(["region"])
                                .build(),
                        )
                        .build(),
                )
                .build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
