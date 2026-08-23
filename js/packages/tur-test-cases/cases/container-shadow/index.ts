import { Alignment, Color, Container, mount, Text, view } from "tur:std";

const App = view(() =>
    Container({
        width: 200,
        height: 200,
        color: Color.hex("#ffffff"),
        borderRadius: 8,
        shadowColor: Color.rgba(0, 0, 0, 80),
        shadowOffset: [4, 4],
        shadowBlur: 12,
        alignment: Alignment.Center,
        children: [Text({ text: "Shadow", fontSize: 16 })],
    }),
);

export function start() {
    mount(App);
}
