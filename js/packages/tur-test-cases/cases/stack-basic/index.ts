import { mount, SizedBox, Stack, view } from "tur:std";

const App = view(() =>
    Stack()
        .children([
            SizedBox().width(100).height(100).build(),
            SizedBox().width(200).height(200).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
