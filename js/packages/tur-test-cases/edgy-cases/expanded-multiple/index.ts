import {
    Column,
    CrossAxisAlignment,
    Expanded,
    SizedBox,
    view,
} from "@tur/edgy";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Expanded({ child: SizedBox({}) }),
            Expanded({ child: SizedBox({}) }),
        ],
    }),
);
