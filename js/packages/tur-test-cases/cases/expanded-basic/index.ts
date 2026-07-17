import {
    Column,
    CrossAxisAlignment,
    Expanded,
    SizedBox,
    view,
} from "builtin:tur/std";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), Expanded({ child: SizedBox({}) })],
    }),
);
