import { view, Text } from "@tur/edgy";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Normal" }, { content: "Italic", italic: true }],
    } as never),
);
