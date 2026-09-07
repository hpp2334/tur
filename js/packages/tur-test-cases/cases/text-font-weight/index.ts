import { Fragment, mount, Text, view } from "tur:std";

const App = view(() =>
    Fragment()
        .children([
            Text({ text: "Hello" }).fontSize(20).fontWeight(400).build(),
            Text({ text: "Hello" }).fontSize(20).fontWeight(700).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
