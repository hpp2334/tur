import { createStore, SizedBox, Text, view } from "tur:std";

export const store = createStore();

export default view(() =>
    SizedBox({ width: 100, height: 50, children: [Text({ text: "Hi" })] }),
);
