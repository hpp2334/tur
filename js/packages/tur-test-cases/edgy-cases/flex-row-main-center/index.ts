import { CrossAxisAlignment, MainAxisAlignment, Row, SizedBox, render } from "@tur/edgy";

render(() =>
    Row({
        mainAlignment: MainAxisAlignment.Center,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
