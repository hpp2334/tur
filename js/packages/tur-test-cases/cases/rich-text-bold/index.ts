import { Text, view } from "tur:std";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Normal" }, { content: "Bold", weight: 700 }],
    } as never),
);
