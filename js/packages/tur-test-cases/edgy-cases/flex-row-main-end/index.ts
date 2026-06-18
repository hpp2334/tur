import {
    CrossAxisAlignment,
    component,
    MainAxisAlignment,
    Row,
    SizedBox,
} from "@tur/edgy";

export default component(() =>
    Row({
        mainAlignment: MainAxisAlignment.End,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
