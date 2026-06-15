import { render, Text } from "@tur/edgy";

render(() =>
    Text({
        text: "Hello World this is a long text that should wrap",
        fontSize: 14,
    }),
);
