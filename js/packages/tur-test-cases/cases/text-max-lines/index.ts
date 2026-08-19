import { createStore, Text, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Text({
        text: "Hello World this is a long text that should wrap",
        fontSize: 14,
        maxLines: 2,
        overflow: "clip",
    }),
);
