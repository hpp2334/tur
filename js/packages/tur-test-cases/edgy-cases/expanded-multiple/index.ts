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
            Expanded({ child: SizedBox({}) }),
            Expanded({ child: SizedBox({}) }),
        ],
    }),
);
