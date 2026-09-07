import {
    Column,
    CrossAxisAlignment,
    Expanded,
    mount,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    Column()
        .crossAlignment(CrossAxisAlignment.Start)
        .children([
            SizedBox().height(50).build(),
            Expanded().child(SizedBox().build()).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
