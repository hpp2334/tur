import {
    Column,
    CrossAxisAlignment,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

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
