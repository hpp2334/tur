import { Column, CrossAxisAlignment, SizedBox, view } from "tur:std";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), SizedBox({ height: 30 })],
    }),
);
