import { CrossAxisAlignment, Row, SizedBox, view } from "builtin:tur/core";

export default view(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
