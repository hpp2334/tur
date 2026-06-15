import { Fragment, render, Text } from "@tur/edgy";

render(() =>
    Fragment({
        children: [
            Text({ text: "Hello", fontSize: 14 }),
            Text({ text: "Hello", fontSize: 28 }),
        ],
    }),
);
