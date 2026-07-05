import {
    CrossAxisAlignment,
    MainAxisAlignment,
    Row,
    SizedBox,
    view,
} from "builtin:tur/std";

export default view(() =>
    Row({
        mainAlignment: MainAxisAlignment.Center,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
