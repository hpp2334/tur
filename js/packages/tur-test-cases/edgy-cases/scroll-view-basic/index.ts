import {
    Column,
    CrossAxisAlignment,
    component,
    ScrollView,
    SizedBox,
} from "@tur/edgy";

export default component(() =>
    ScrollView({
        queryKey: ["scroll-view"],
        child: Column({
            crossAlignment: CrossAxisAlignment.Start,
            children: [
                SizedBox({ height: 200 }),
                SizedBox({ height: 200 }),
                SizedBox({ height: 200 }),
            ],
        }),
    }),
);
