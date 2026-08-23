import { mount, Text, view } from "tur:std";

const App = view(() =>
    Text({
        text: "Hello World this is a long text that should wrap",
        fontSize: 14,
        maxLines: 2,
        overflow: "clip",
    }),
);

export function start() {
    mount(App);
}
