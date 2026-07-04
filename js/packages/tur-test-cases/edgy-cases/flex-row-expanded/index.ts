import {
    CrossAxisAlignment,
    Expanded,
    Row,
    SizedBox,
    view,
} from "builtin:tur/core";

export default view(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), Expanded({ child: SizedBox({}) })],
    }),
);
