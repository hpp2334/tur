import { CrossAxisAlignment, component, Row, SizedBox } from "@tur/edgy";

export default component(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
