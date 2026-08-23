import { mount, Text, view } from "tur:std";

const App = view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Small" }, { content: "Big", fontSize: 28 }],
    } as never),
);

export function start() {
    mount(App);
}
