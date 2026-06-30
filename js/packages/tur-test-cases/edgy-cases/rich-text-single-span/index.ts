import { view, Text } from "@tur/edgy";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "Hello World" }],
    } as never),
);
