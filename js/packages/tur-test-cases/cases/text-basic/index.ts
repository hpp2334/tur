import { mount, Text, view } from "tur:std";

const App = view(() => Text({ text: "Hello" }).fontSize(14).build());

export function start() {
    mount(App);
}
