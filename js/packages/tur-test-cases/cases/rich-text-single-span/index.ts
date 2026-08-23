import { mount, Text, view } from "tur:std";

const App = view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Hello World" }],
    } as never),
);

export function start() {
    mount(App);
}
