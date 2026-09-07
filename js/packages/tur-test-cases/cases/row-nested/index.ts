import { CrossAxisAlignment, mount, Row, SizedBox, view } from "tur:std";

// Row-in-Row (Flutter parity): non-flex children of a flex receive unbounded
// main-axis constraints, so the inner Row (default MainAxisSize.max) must
// shrink-wrap to its content width instead of consuming the outer Row's full
// width — leaving the following sibling visible instead of pushed past the
// outer extent.
const App = view(() =>
    Row()
        .crossAlignment(CrossAxisAlignment.Start)
        .children([
            Row()
                .children([
                    SizedBox().width(20).build(),
                    SizedBox().width(30).build(),
                ])
                .build(),
            SizedBox().width(50).height(10).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
