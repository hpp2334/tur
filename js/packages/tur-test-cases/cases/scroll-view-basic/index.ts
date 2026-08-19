import {
    Column,
    CrossAxisAlignment,
    createStore,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

export const store = createStore();

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
