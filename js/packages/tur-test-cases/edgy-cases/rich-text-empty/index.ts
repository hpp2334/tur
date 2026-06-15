import { Text, render } from "@tur/edgy";

render(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "" }],
    } as never),
);
