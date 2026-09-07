import { mount, Text, view } from "tur:std";

const App = view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "" }],
    } as never).build(),
);

export function start() {
    mount(App);
}
