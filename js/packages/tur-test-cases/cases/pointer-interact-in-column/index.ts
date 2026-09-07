import {
    Column,
    Container,
    CrossAxisAlignment,
    mount,
    PointerInteract,
    view,
} from "tur:std";

const App = view(() =>
    Column()
        .crossAlignment(CrossAxisAlignment.Start)
        .children([
            PointerInteract()
                .child(Container().width(80).height(40).build())
                .build(),
            PointerInteract()
                .child(Container().width(60).height(30).build())
                .build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
