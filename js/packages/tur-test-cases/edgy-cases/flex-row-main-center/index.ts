import {
    CrossAxisAlignment,
    MainAxisAlignment,
    Row,
    SizedBox,
    view,
} from "@tur/edgy";

export default view(() =>
    Row({
        mainAlignment: MainAxisAlignment.Center,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
