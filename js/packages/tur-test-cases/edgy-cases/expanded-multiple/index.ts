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
            Expanded({ child: SizedBox({}) }),
            Expanded({ child: SizedBox({}) }),
        ],
    }),
);
