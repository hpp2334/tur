import { Fragment, Text, view } from "@tur/edgy";

export default view(() =>
    Fragment({
        children: [
            Text({ text: "Hello", fontSize: 14 }),
            Text({ text: "Hello", fontSize: 28 }),
        ],
    }),
);
