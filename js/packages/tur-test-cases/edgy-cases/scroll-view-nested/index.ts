import {
    Column,
    Container,
    CrossAxisAlignment,
    Row,
    render,
    ScrollView,
    SizedBox,
} from "@tur/edgy";

render(() =>
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
