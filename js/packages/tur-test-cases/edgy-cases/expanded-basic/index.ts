import {
    Column,
    CrossAxisAlignment,
    component,
    Expanded,
    SizedBox,
} from "@tur/edgy";

export default component(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), Expanded({ child: SizedBox({}) })],
    }),
);
