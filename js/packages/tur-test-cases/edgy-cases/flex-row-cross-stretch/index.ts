import { CrossAxisAlignment, Row, SizedBox, view } from "@tur/edgy";

export default view(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
