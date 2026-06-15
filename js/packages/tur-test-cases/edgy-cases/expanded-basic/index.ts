import { Expanded, Column, CrossAxisAlignment, SizedBox, render } from "@tur/edgy";

render(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), Expanded({ child: SizedBox({}) })],
    }),
);
