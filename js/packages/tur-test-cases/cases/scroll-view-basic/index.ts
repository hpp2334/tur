import {
    Column,
    CrossAxisAlignment,
    mount,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    ScrollView()
        .queryKey(["scroll-view"])
        .child(
            Column()
                .crossAlignment(CrossAxisAlignment.Stretch)
                .children([
                    SizedBox().height(200).build(),
                    SizedBox().height(200).build(),
                    SizedBox().height(200).build(),
                ])
                .build(),
        )
        .build(),
);

export function start() {
    mount(App);
}
