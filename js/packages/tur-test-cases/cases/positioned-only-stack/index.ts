import { Expanded, mount, Positioned, SizedBox, Stack, view } from "tur:std";

// A Stack with ONLY positioned children sizes itself to the biggest size
// the incoming constraints allow (Flutter RenderStack:
// `size = constraints.biggest` when there are no non-positioned children)
// — this is what gives right/bottom anchors a reference box.
const App = view(() =>
    Expanded({
        child: Stack({
            children: [
                Positioned({
                    left: 5,
                    top: 5,
                    child: SizedBox({
                        width: 10,
                        height: 10,
                        queryKey: ["dot"],
                    }),
                }),
            ],
        }),
    }),
);

export function start() {
    mount(App);
}
