import {
    Column,
    CrossAxisAlignment,
    MainAxisSize,
    mount,
    Row,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    Column()
        .crossAlignment(CrossAxisAlignment.Start)
        .children([
            Row()
                .crossAlignment(CrossAxisAlignment.Center)
                .mainAxisSize(MainAxisSize.Min)
                .children([
                    SizedBox().width(20).height(20).build(),
                    SizedBox().width(40).height(10).build(),
                ])
                .build(),
            SizedBox().height(30).build(),
            SizedBox().height(20).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
