import {
    CrossAxisAlignment,
    Expanded,
    mount,
    Row,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    Row()
        .crossAlignment(CrossAxisAlignment.Start)
        .children([
            SizedBox().width(50).build(),
            Expanded().child(SizedBox().build()).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
