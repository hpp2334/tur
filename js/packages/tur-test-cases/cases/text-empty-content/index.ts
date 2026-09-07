import { mount, Text, view } from "tur:std";

const App = view(() => Text({ text: "" }).build());

export function start() {
    mount(App);
}
