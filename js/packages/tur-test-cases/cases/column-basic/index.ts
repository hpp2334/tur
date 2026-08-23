import { Column, CrossAxisAlignment, mount, SizedBox, view } from "tur:std";

const App = view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), SizedBox({ height: 30 })],
    }),
);

export function start() {
    mount(App);
}
