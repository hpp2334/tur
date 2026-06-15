import { Column, Container, Container as C2, CrossAxisAlignment, Row, SizedBox, render } from "@tur/edgy";

render(() =>
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
