import { Text, view } from "@tur/edgy";

export default view(() =>
    Text({
        fontSize: 14,
        spans: [{ content: "" }],
    } as never),
);
