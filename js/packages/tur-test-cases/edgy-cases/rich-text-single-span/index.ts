import { Text, view } from "builtin:tur/std";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Hello World" }],
    } as never),
);
