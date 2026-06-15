import { Column, CrossAxisAlignment, SizedBox, render } from "@tur/edgy";

render(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 100, height: 50 })],
    }),
);
