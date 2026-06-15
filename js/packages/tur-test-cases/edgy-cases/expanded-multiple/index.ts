import {
    Column,
    CrossAxisAlignment,
    Expanded,
    render,
    SizedBox,
} from "@tur/edgy";

render(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Expanded({ child: SizedBox({}) }),
            Expanded({ child: SizedBox({}) }),
        ],
    }),
);
