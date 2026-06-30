import {
    Column,
    CrossAxisAlignment,
    view,
    Expanded,
    SizedBox,
} from "@tur/edgy";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Expanded({ flex: 2, child: SizedBox({}) }),
            Expanded({ flex: 1, child: SizedBox({}) }),
        ],
    }),
);
