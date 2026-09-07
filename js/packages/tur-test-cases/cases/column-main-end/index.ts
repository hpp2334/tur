import {
    Column,
    CrossAxisAlignment,
    MainAxisAlignment,
    mount,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    Column()
        .mainAlignment(MainAxisAlignment.End)
        .crossAlignment(CrossAxisAlignment.Start)
        .children([
            SizedBox().height(50).build(),
            SizedBox().height(30).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
