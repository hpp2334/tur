import {
    CrossAxisAlignment,
    MainAxisAlignment,
    mount,
    Row,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    Row()
        .mainAlignment(MainAxisAlignment.End)
        .crossAlignment(CrossAxisAlignment.Start)
        .children([SizedBox().width(50).build(), SizedBox().width(30).build()])
        .build(),
);

export function start() {
    mount(App);
}
