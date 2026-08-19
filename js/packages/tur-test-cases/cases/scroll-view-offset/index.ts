import {
    Column,
    CrossAxisAlignment,
    createScrollController,
    createStore,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

export const store = createStore();

const controller = createScrollController({ initialOffset: 100 });

export default view(() =>
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
