import { createStore, Fragment, Text, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Fragment({
        children: [
            Text({ text: "Hello", fontSize: 20, fontWeight: 400 }),
            Text({ text: "Hello", fontSize: 20, fontWeight: 700 }),
        ],
    }),
);
