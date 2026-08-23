import { Column, Container, mount, view } from "tur:std";

const App = view(() =>
    Column({
        children: [
            Container({ width: 200, height: 50 }),
            Container({ width: 200, height: 30 }),
        ],
    }),
);

export function start() {
    mount(App);
}
