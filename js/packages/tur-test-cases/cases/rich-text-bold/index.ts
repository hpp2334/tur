import { Text, view } from "builtin:tur/std";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Normal" }, { content: "Bold", bold: true }],
    } as never),
);
