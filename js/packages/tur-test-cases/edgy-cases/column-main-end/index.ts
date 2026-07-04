import {
    Column,
    CrossAxisAlignment,
    MainAxisAlignment,
    SizedBox,
    view,
} from "builtin:tur/core";

export default view(() =>
    Column({
        mainAlignment: MainAxisAlignment.End,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), SizedBox({ height: 30 })],
    }),
);
