import { createStore, Fragment, Text, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Fragment({
        children: [
            Text({ text: "Hello", fontSize: 14 }),
            Text({ text: "Hello", fontSize: 28 }),
        ],
    }),
);
