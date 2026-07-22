import { Color, Text, view } from "tur:std";

export default view(() =>
    Text({
        fontSize: 20,
        spans: [
            { content: "Inherited", color: Color.hex("#ff0000") },
            { content: "Override", fontSize: 10, color: Color.hex("#00ff00") },
        ],
    } as never),
);
