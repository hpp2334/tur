import { view, Text } from "@tur/edgy";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [
            { content: "Hello " },
            { content: "Bold", bold: true },
            { content: " World" },
        ],
    } as never),
);
