import { CrossAxisAlignment, Expanded, Row, SizedBox, render } from "@tur/edgy";

render(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            SizedBox({ width: 50 }),
            Expanded({ child: SizedBox({}) }),
        ],
    }),
);
