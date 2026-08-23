import { Column, MainAxisSize, mount, SizedBox, view } from "tur:std";

// Three 300px children in a Min column — total 900px overflows the 600px
// viewport. Children should keep their natural height (not squish to 0).
const App = view(() =>
    Column({
        mainAxisSize: MainAxisSize.Min,
        children: [
            SizedBox({ height: 300 }),
            SizedBox({ height: 300 }),
            SizedBox({ height: 300 }),
        ],
    }),
);

export function start() {
    mount(App);
}
