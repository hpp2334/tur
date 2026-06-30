import {
    Column,
    CrossAxisAlignment,
    view,
    ScrollView,
    SizedBox,
} from "@tur/edgy";

export default view(() =>
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
