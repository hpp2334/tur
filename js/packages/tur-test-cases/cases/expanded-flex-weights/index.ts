import { Column, CrossAxisAlignment, Expanded, SizedBox, view } from "tur:std";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Expanded({ flex: 2, child: SizedBox({}) }),
            Expanded({ flex: 1, child: SizedBox({}) }),
        ],
    }),
);
