import { Color, mount, Text, view } from "tur:std";

const App = view(() =>
    Text({
        fontSize: 14,
        spans: [
            { content: "White" },
            { content: "Red", color: Color.hex("#ff0000") },
            { content: "Green", color: Color.hex("#00ff00") },
        ],
    } as never),
);

export function start() {
    mount(App);
}
