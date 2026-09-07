import { Container, mount, SizedBox, view } from "tur:std";

const App = view(() =>
    Container()
        .padding(16)
        .children([SizedBox().width(100).height(100).build()])
        .build(),
);

export function start() {
    mount(App);
}
