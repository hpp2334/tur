import { Fragment, mount, Text, view } from "tur:std";

const App = view(() =>
    Fragment()
        .children([
            Text({ text: "Hello" }).fontSize(14).build(),
            Text({ text: "Hello" }).fontSize(28).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
