import { component, Text } from "@tur/edgy";

export default component(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "" }],
    } as never),
);
