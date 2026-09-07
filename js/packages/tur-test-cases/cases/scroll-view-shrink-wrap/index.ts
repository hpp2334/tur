import {
    Column,
    CrossAxisAlignment,
    mount,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

// Flutter parity (_RenderSingleChildViewport): a ScrollView sizes itself to
// `constraints.constrain(child.size)` — shrink-wrapped to its content on BOTH
// axes, clamped by the incoming constraints. Under the root's loose
// 400×600 constraints the viewport must be the content size (120×80), not the
// constraint maxes. (Filling requires tight constraints — e.g. `Expanded`.)
const App = view(() =>
    ScrollView()
        .queryKey(["sv"])
        .child(
            Column()
                .crossAlignment(CrossAxisAlignment.Start)
                .children([
                    SizedBox().width(120).height(40).build(),
                    SizedBox().width(60).height(40).build(),
                ])
                .build(),
        )
        .build(),
);

export function start() {
    mount(App);
}
