import { ClipBehavior, Color, Container, mount, view } from "tur:std";

const App = view(() =>
    Container({
        width: 200,
        height: 200,
        borderRadius: 40,
        clipBehavior: ClipBehavior.HardEdge,
        children: [
            Container({
                width: 200,
                height: 200,
                color: Color.hex("#ff0000"),
            }),
        ],
    }),
);

export function start() {
    mount(App);
}
