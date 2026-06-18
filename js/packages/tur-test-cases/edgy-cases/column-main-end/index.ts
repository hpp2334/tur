import {
    Column,
    CrossAxisAlignment,
    component,
    MainAxisAlignment,
    SizedBox,
} from "@tur/edgy";

export default component(() =>
    Column({
        mainAlignment: MainAxisAlignment.End,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), SizedBox({ height: 30 })],
    }),
);
