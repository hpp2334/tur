import {
    Column,
    Container,
    CrossAxisAlignment,
    mount,
    Row,
    ScrollView,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    Row({
        children: [
            SizedBox({ width: 200 }),
            ScrollView({
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
                                    child: Column({
                                        crossAlignment:
                                            CrossAxisAlignment.Start,
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
        ],
    }),
);

export function start() {
    mount(App);
}
