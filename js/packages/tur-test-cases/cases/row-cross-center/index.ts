import {
    Container,
    CrossAxisAlignment,
    mount,
    Row,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    Container()
        .height(36)
        .width(200)
        .children([
            Row()
                .crossAlignment(CrossAxisAlignment.Center)
                .children([
                    SizedBox().width(20).height(20).build(),
                    SizedBox().width(40).height(10).build(),
                ])
                .build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
