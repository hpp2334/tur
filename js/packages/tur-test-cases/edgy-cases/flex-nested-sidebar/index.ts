import {
    Container as C2,
    Column,
    Container,
    CrossAxisAlignment,
    component,
    Row,
    SizedBox,
} from "@tur/edgy";

export default component(() =>
    Row({
        children: [
            Container({
                width: 200,
                children: [
                    Column({
                        crossAlignment: CrossAxisAlignment.Start,
                        children: [SizedBox({ height: 40 })],
                    }),
                ],
            }),
            Container({
                children: [
                    Column({
                        crossAlignment: CrossAxisAlignment.Start,
                        children: [SizedBox({ height: 20 })],
                    }),
                ],
            }),
        ],
    }),
);
