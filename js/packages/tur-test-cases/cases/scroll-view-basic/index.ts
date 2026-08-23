import {
    Column,
    CrossAxisAlignment,
    mount,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
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

export function start() {
    mount(App);
}
