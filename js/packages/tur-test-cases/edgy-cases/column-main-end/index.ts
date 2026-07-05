import {
    Column,
    CrossAxisAlignment,
    MainAxisAlignment,
    SizedBox,
    view,
} from "builtin:tur/std";

export default view(() =>
    Column({
        mainAlignment: MainAxisAlignment.End,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), SizedBox({ height: 30 })],
    }),
);
