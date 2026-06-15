import {
    Column,
    CrossAxisAlignment,
    MainAxisAlignment,
    SizedBox,
    render,
} from "@tur/edgy";

render(() =>
    Column({
        mainAlignment: MainAxisAlignment.End,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), SizedBox({ height: 30 })],
    }),
);
