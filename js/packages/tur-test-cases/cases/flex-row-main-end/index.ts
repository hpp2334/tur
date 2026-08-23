import {
    CrossAxisAlignment,
    MainAxisAlignment,
    Row,
    SizedBox,
    view,
} from "tur:std";

export default view(() =>
    Row({
        mainAlignment: MainAxisAlignment.End,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
