import { render, Text } from "@tur/edgy";

render(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "" }],
    } as never),
);
