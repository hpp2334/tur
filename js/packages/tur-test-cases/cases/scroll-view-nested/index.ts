import {
    Column,
    Container,
    CrossAxisAlignment,
    Expanded,
    mount,
    Row,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

// The outer ScrollView sits in a Row (horizontal flex). Non-flex children of
// a flex get unbounded MAIN-axis constraints (Flutter RenderFlex parity), so
// a vertical ScrollView there has unbounded width — `Expanded` is what gives
// it the bounded viewport, exactly as in Flutter.
const App = view(() =>
    Row()
        .children([
            SizedBox().width(200).build(),
            Expanded()
                .child(
                    ScrollView()
                        .queryKey(["outer-scroll"])
                        .child(
                            Column()
                                .crossAlignment(CrossAxisAlignment.Start)
                                .children([
                                    SizedBox().height(100).build(),
                                    Container()
                                        .height(200)
                                        .queryKey(["inner-wrapper"])
                                        .children([
                                            ScrollView()
                                                .queryKey(["inner-scroll"])
                                                .child(
                                                    Column()
                                                        .crossAlignment(
                                                            CrossAxisAlignment.Stretch,
                                                        )
                                                        .children([
                                                            SizedBox()
                                                                .height(200)
                                                                .build(),
                                                            SizedBox()
                                                                .height(200)
                                                                .build(),
                                                            SizedBox()
                                                                .height(200)
                                                                .build(),
                                                        ])
                                                        .build(),
                                                )
                                                .build(),
                                        ])
                                        .build(),
                                    SizedBox().height(400).build(),
                                ])
                                .build(),
                        )
                        .build(),
                )
                .build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
