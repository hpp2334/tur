import {
    Container as C2,
    Column,
    Container,
    CrossAxisAlignment,
    view,
    Row,
    SizedBox,
} from "@tur/edgy";

export default view(() =>
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
