import { component, Text } from "@tur/edgy";

export default component(() =>
    Text({
        text: "Hello World this is a long text that should wrap",
        fontSize: 14,
    }),
);
