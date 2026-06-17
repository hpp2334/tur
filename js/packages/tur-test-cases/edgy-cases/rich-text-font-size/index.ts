import { component, Text } from "@tur/edgy";

export default component(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Small" }, { content: "Big", fontSize: 28 }],
    } as never),
);
