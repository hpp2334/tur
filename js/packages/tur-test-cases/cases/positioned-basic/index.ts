import { mount, Positioned, SizedBox, Stack, view } from "tur:std";

const App = view(() =>
    Stack({
        children: [
            Positioned({
                left: 10,
                top: 20,
                child: SizedBox({ width: 50, height: 50 }),
            }),
        ],
    }),
);

export function start() {
    mount(App);
}
