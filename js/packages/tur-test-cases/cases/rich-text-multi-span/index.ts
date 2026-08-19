import { createStore, Text, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Text({
        fontSize: 14,
        spans: [
            { content: "Hello " },
            { content: "Bold", weight: 700 },
            { content: " World" },
        ],
    } as never),
);
