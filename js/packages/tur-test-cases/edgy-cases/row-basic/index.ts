import { Row, CrossAxisAlignment, SizedBox, render } from "@tur/edgy";

render(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
