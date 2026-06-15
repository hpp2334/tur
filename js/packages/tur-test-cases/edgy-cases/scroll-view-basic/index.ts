import {
    Column,
    CrossAxisAlignment,
    render,
    ScrollView,
    SizedBox,
} from "@tur/edgy";

render(() =>
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
