import { CrossAxisAlignment, view, Row, SizedBox } from "@tur/edgy";

export default view(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
