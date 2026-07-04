import { Color, Text, view } from "builtin:tur/core";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [
            { content: "White" },
            { content: "Red", color: Color.hex("#ff0000") },
            { content: "Green", color: Color.hex("#00ff00") },
        ],
    } as never),
);
