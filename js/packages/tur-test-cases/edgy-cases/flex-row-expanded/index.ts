import {
    CrossAxisAlignment,
    view,
    Expanded,
    Row,
    SizedBox,
} from "@tur/edgy";

export default view(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), Expanded({ child: SizedBox({}) })],
    }),
);
