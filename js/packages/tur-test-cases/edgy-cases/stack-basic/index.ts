import { Stack, SizedBox, render } from "@tur/edgy";

render(() =>
    Stack({
        children: [SizedBox({ width: 100, height: 100 }), SizedBox({ width: 200, height: 200 })],
    }),
);
