import { render, Text } from "@tur/edgy";

render(() =>
    Text({
        fontSize: 14,
        spans: [
            { content: "Hello " },
            { content: "Bold", bold: true },
            { content: " World" },
        ],
    } as never),
);
