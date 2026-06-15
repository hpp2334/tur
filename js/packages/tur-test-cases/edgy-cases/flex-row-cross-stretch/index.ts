import { CrossAxisAlignment, Row, SizedBox, render } from "@tur/edgy";

render(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
