import { Text, view } from "builtin:tur/std";

export default view(() =>
    Text({
        text: "Hello World this is a long text that should wrap",
        fontSize: 14,
    }),
);
