import { createStore, Text, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Small" }, { content: "Big", fontSize: 28 }],
    } as never),
);
