import {
    Column,
    CrossAxisAlignment,
    Expanded,
    SizedBox,
    view,
} from "builtin:tur/core";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Expanded({ flex: 2, child: SizedBox({}) }),
            Expanded({ flex: 1, child: SizedBox({}) }),
        ],
    }),
);
