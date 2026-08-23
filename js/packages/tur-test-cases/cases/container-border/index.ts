import {
    Alignment,
    BorderPosition,
    Color,
    Container,
    mount,
    Text,
    view,
} from "tur:std";

const App = view(() =>
    Container({
        width: 200,
        height: 200,
        padding: 16,
        color: Color.hex("#ffffff"),
        borderColor: Color.hex("#000000"),
        borderWidth: 2,
        borderRadius: 8,
        borderPosition: BorderPosition.Inside,
        alignment: Alignment.Center,
        children: [Text({ text: "Border", fontSize: 16 })],
    }),
);

export function start() {
    mount(App);
}
