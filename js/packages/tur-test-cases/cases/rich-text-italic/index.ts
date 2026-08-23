import { mount, Text, view } from "tur:std";

const App = view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Normal" }, { content: "Italic", italic: true }],
    } as never),
);

export function start() {
    mount(App);
}
