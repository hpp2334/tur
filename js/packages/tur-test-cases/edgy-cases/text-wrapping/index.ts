import { Text, view } from "@tur/edgy";

export default view(() =>
    Text({
        text: "Hello World this is a long text that should wrap",
        fontSize: 14,
    }),
);
