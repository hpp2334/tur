import {
    CrossAxisAlignment,
    view,
    MainAxisAlignment,
    Row,
    SizedBox,
} from "@tur/edgy";

export default view(() =>
    Row({
        mainAlignment: MainAxisAlignment.End,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
