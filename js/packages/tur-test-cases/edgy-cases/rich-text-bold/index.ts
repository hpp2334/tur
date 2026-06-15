import { render, Text } from "@tur/edgy";

render(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Normal" }, { content: "Bold", bold: true }],
    } as never),
);
