import { CrossAxisAlignment, mount, Row, SizedBox, view } from "tur:std";

const App = view(() =>
    Row()
        .crossAlignment(CrossAxisAlignment.Stretch)
        .children([SizedBox().width(50).build(), SizedBox().width(30).build()])
        .build(),
);

export function start() {
    mount(App);
}
