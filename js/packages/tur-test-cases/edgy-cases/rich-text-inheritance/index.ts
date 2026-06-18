import { Color, component, Text } from "@tur/edgy";

export default component(() =>
    Text({
        fontSize: 20,
        spans: [
            { content: "Inherited", color: Color.hex("#ff0000") },
            { content: "Override", fontSize: 10, color: Color.hex("#00ff00") },
        ],
    } as never),
);
