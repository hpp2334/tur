import { Column, CrossAxisAlignment, createStore, Text, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.End,
        children: [
            Text({ text: "First", fontSize: 14 }),
            Text({ text: "Second", fontSize: 14 }),
        ],
    }),
);
