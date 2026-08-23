import { mount, Text, view } from "tur:std";

const App = view(() =>
    Text({
        fontSize: 14,
        spans: [
            { content: "Hello " },
            { content: "Bold", weight: 700 },
            { content: " World" },
        ],
    } as never),
);

export function start() {
    mount(App);
}
