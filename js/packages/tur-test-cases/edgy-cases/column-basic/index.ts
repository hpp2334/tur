import { Column, CrossAxisAlignment, SizedBox, view } from "@tur/edgy";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), SizedBox({ height: 30 })],
    }),
);
