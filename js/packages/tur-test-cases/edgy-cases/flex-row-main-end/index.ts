import {
    CrossAxisAlignment,
    MainAxisAlignment,
    Row,
    render,
    SizedBox,
} from "@tur/edgy";

render(() =>
    Row({
        mainAlignment: MainAxisAlignment.End,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
