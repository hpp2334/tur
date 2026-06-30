import { Column, CrossAxisAlignment, SizedBox, view } from "@tur/edgy";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 100, height: 50 })],
    }),
);
