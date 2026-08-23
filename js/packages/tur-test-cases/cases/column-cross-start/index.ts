import { Column, CrossAxisAlignment, mount, SizedBox, view } from "tur:std";

const App = view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 100, height: 50 })],
    }),
);

export function start() {
    mount(App);
}
