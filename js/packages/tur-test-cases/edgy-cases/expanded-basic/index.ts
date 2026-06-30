import {
    Column,
    CrossAxisAlignment,
    Expanded,
    SizedBox,
    view,
} from "@tur/edgy";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), Expanded({ child: SizedBox({}) })],
    }),
);
