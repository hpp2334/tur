import { CrossAxisAlignment, mount, Row, SizedBox, view } from "tur:std";

// Row-in-Row (Flutter parity): non-flex children of a flex receive unbounded
// main-axis constraints, so the inner Row (default MainAxisSize.max) must
// shrink-wrap to its content width instead of consuming the outer Row's full
// width — leaving the following sibling visible instead of pushed past the
// outer extent.
const App = view(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Row({
                children: [SizedBox({ width: 20 }), SizedBox({ width: 30 })],
            }),
            SizedBox({ width: 50, height: 10 }),
        ],
    }),
);

export function start() {
    mount(App);
}
