import { Expanded, Column, CrossAxisAlignment, SizedBox, render } from "@tur/edgy";

render(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Expanded({ child: SizedBox({}) }),
            Expanded({ child: SizedBox({}) }),
        ],
    }),
);
