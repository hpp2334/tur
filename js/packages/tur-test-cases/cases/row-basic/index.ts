import { CrossAxisAlignment, Row, SizedBox, view } from "tur:std";

export default view(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
