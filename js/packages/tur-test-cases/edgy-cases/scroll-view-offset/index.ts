import {
    Column,
    CrossAxisAlignment,
    createScrollController,
    ScrollView,
    SizedBox,
    render,
} from "@tur/edgy";

const controller = createScrollController({ initialOffset: 100 });

render(() =>
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
