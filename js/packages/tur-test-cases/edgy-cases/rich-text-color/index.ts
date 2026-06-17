import { Color, component, Text } from "@tur/edgy";

export default component(() =>
    Text({
        fontSize: 14,
        spans: [
            { content: "White" },
            { content: "Red", color: Color.hex("#ff0000") },
            { content: "Green", color: Color.hex("#00ff00") },
        ],
    } as never),
);
