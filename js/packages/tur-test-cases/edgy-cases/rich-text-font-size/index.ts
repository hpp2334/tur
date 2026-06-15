import { Text, render } from "@tur/edgy";

render(() =>
    Text({
        fontSize: 14,
        spans: [
            { content: "Small" },
            { content: "Big", fontSize: 28 },
        ],
    } as never),
);
