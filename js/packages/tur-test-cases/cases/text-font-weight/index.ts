import { Fragment, mount, Text, view } from "tur:std";

const App = view(() =>
    Fragment({
        children: [
            Text({ text: "Hello", fontSize: 20, fontWeight: 400 }),
            Text({ text: "Hello", fontSize: 20, fontWeight: 700 }),
        ],
    }),
);

export function start() {
    mount(App);
}
