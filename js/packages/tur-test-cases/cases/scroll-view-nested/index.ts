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
    Row({
        children: [
            SizedBox({ width: 200 }),
            Expanded({
                child: ScrollView({
                    queryKey: ["outer-scroll"],
                    child: Column({
                        crossAlignment: CrossAxisAlignment.Start,
                        children: [
                            SizedBox({ height: 100 }),
                            Container({
                                height: 200,
                                queryKey: ["inner-wrapper"],
                                children: [
                                    ScrollView({
                                        queryKey: ["inner-scroll"],
                                        // Stretch content so the inner
                                        // viewport is full-width (the
                                        // ScrollView shrink-wraps to its
                                        // content — Flutter parity).
                                        child: Column({
                                            crossAlignment:
                                                CrossAxisAlignment.Stretch,
                                            children: [
                                                SizedBox({ height: 200 }),
                                                SizedBox({ height: 200 }),
                                                SizedBox({ height: 200 }),
                                            ],
                                        }),
                                    }),
                                ],
                            }),
                            SizedBox({ height: 400 }),
                        ],
                    }),
                }),
            }),
        ],
    }),
);

export function start() {
    mount(App);
}
