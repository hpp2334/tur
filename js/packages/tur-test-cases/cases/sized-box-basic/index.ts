import { mount, SizedBox, Text, view } from "tur:std";

const App = view(() =>
    SizedBox({ width: 100, height: 50, children: [Text({ text: "Hi" })] }),
);

export function start() {
    mount(App);
}
