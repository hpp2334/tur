import { component, Text } from "@tur/edgy";

export default component(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Normal" }, { content: "Italic", italic: true }],
    } as never),
);
