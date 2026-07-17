import { Text, view } from "builtin:tur/std";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Small" }, { content: "Big", fontSize: 28 }],
    } as never),
);
