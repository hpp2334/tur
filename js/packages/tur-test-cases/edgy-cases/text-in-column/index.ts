import { Column, CrossAxisAlignment, render, Text } from "@tur/edgy";

render(() =>
    Column({
        crossAlignment: CrossAxisAlignment.End,
        children: [
            Text({ text: "First", fontSize: 14 }),
            Text({ text: "Second", fontSize: 14 }),
        ],
    }),
);
