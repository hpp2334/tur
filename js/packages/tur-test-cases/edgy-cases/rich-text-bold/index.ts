import { Text, view } from "builtin:tur/core";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Normal" }, { content: "Bold", bold: true }],
    } as never),
);
