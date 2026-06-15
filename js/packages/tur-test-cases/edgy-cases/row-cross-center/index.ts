import {
    Container,
    CrossAxisAlignment,
    Row,
    render,
    SizedBox,
} from "@tur/edgy";

render(() =>
    Container({
        height: 36,
        width: 200,
        children: [
            Row({
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    SizedBox({ width: 20, height: 20 }),
                    SizedBox({ width: 40, height: 10 }),
                ],
            }),
        ],
    }),
);
