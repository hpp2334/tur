import {
    Column,
    CrossAxisAlignment,
    Expanded,
    SizedBox,
    view,
} from "builtin:tur/std";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Expanded({ child: SizedBox({}) }),
            Expanded({ child: SizedBox({}) }),
        ],
    }),
);
