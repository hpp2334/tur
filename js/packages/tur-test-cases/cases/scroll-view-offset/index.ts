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
    ScrollView()
        .controller(controller)
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
