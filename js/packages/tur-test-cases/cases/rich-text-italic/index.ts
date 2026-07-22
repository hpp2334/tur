import { Text, view } from "tur:std";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Normal" }, { content: "Italic", italic: true }],
    } as never),
);
