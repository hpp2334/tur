import {
    Column,
    CrossAxisAlignment,
    view,
    Expanded,
    SizedBox,
} from "@tur/edgy";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), Expanded({ child: SizedBox({}) })],
    }),
);
