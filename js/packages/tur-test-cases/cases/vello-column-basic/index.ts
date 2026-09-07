import { Column, Container, mount, view } from "tur:std";

const App = view(() =>
    Column()
        .children([
            Container().width(200).height(50).build(),
            Container().width(200).height(30).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
