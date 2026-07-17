import { Column, CrossAxisAlignment, Text, view } from "builtin:tur/std";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.End,
        children: [
            Text({ text: "First", fontSize: 14 }),
            Text({ text: "Second", fontSize: 14 }),
        ],
    }),
);
