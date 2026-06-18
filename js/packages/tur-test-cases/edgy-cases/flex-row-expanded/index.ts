import {
    CrossAxisAlignment,
    component,
    Expanded,
    Row,
    SizedBox,
} from "@tur/edgy";

export default component(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), Expanded({ child: SizedBox({}) })],
    }),
);
