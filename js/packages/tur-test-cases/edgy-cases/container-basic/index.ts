import { Container, component, SizedBox } from "@tur/edgy";

export default component(() =>
    Container({
        padding: 16,
        children: [SizedBox({ width: 100, height: 100 })],
    }),
);
