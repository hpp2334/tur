import { mount, Text, view } from "tur:std";

const App = view(() => Text({ text: "Hello", fontSize: 14 }));

export function start() {
    mount(App);
}
