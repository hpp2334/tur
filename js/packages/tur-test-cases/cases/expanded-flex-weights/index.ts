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
            Expanded().flex(2).child(SizedBox().build()).build(),
            Expanded().flex(1).child(SizedBox().build()).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
