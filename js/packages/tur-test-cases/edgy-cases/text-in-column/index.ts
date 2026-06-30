import { Column, CrossAxisAlignment, view, Text } from "@tur/edgy";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.End,
        children: [
            Text({ text: "First", fontSize: 14 }),
            Text({ text: "Second", fontSize: 14 }),
        ],
    }),
);
