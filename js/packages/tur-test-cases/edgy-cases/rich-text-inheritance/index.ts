import { Color, Text, render } from "@tur/edgy";

render(() =>
    Text({
        fontSize: 20,
        spans: [
            { content: "Inherited", color: Color.hex("#ff0000") },
            { content: "Override", fontSize: 10, color: Color.hex("#00ff00") },
        ],
    } as never),
);
