import {
    Column,
    Container,
    CrossAxisAlignment,
    view,
    Row,
    ScrollView,
    SizedBox,
} from "@tur/edgy";

export default view(() =>
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
