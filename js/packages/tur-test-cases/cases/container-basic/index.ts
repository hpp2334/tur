import { Container, SizedBox, view } from "builtin:tur/std";

export default view(() =>
    Container({
        padding: 16,
        children: [SizedBox({ width: 100, height: 100 })],
    }),
);
