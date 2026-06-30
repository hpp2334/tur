import {
    Column,
    CrossAxisAlignment,
    MainAxisAlignment,
    SizedBox,
    view,
} from "@tur/edgy";

export default view(() =>
    Column({
        mainAlignment: MainAxisAlignment.End,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), SizedBox({ height: 30 })],
    }),
);
