import { Color, Container, mount, Positioned, Stack, view } from "tur:std";

const App = view(() =>
    Stack()
        .children([
            Positioned()
                .left(0)
                .top(0)
                .child(
                    Container()
                        .width(100)
                        .height(100)
                        .color(Color.hex("#ff0000"))
                        .build(),
                )
                .build(),
            Positioned()
                .left(100)
                .top(0)
                .child(
                    Container()
                        .width(100)
                        .height(100)
                        .color(Color.hex("#00ff00"))
                        .build(),
                )
                .build(),
            Positioned()
                .left(0)
                .top(100)
                .child(
                    Container()
                        .width(100)
                        .height(100)
                        .color(Color.hex("#0000ff"))
                        .build(),
                )
                .build(),
            Positioned()
                .left(100)
                .top(100)
                .child(
                    Container()
                        .width(100)
                        .height(100)
                        .color(Color.hex("#ffff00"))
                        .build(),
                )
                .build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
