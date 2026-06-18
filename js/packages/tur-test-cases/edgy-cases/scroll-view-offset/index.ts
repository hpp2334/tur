import {
    Column,
    CrossAxisAlignment,
    component,
    createScrollController,
    ScrollView,
    SizedBox,
} from "@tur/edgy";

const controller = createScrollController({ initialOffset: 100 });

export default component(() =>
    ScrollView({
        controller,
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
