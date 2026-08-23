import { Column, CrossAxisAlignment, mount, Text, view } from "tur:std";

const App = view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.End,
        children: [
            Text({ text: "First", fontSize: 14 }),
            Text({ text: "Second", fontSize: 14 }),
        ],
    }),
);

export function start() {
    mount(App);
}
