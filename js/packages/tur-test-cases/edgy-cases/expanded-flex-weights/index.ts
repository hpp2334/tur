import {
    Column,
    CrossAxisAlignment,
    component,
    Expanded,
    SizedBox,
} from "@tur/edgy";

export default component(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Expanded({ flex: 2, child: SizedBox({}) }),
            Expanded({ flex: 1, child: SizedBox({}) }),
        ],
    }),
);
