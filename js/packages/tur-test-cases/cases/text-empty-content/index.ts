import { mount, Text, view } from "tur:std";

const App = view(() => Text({ text: "" }));

export function start() {
    mount(App);
}
