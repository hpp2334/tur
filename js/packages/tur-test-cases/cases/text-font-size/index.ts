import { Fragment, mount, Text, view } from "tur:std";

const App = view(() =>
    Fragment({
        children: [
            Text({ text: "Hello", fontSize: 14 }),
            Text({ text: "Hello", fontSize: 28 }),
        ],
    }),
);

export function start() {
    mount(App);
}
