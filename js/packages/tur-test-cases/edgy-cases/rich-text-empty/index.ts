import { Text, view } from "builtin:tur/core";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "" }],
    } as never),
);
