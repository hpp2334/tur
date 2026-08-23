import { mount, Text, view } from "tur:std";

const App = view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Normal" }, { content: "Bold", weight: 700 }],
    } as never),
);

export function start() {
    mount(App);
}
