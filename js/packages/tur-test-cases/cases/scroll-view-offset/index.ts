import {
    Column,
    CrossAxisAlignment,
    createScrollController,
    mount,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

const controller = createScrollController({ initialOffset: 100 });

const App = view(() =>
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

export function start() {
    mount(App);
}
