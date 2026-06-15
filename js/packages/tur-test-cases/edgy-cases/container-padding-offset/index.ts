import {
    Container,
    CrossAxisAlignment,
    Row,
    render,
    SizedBox,
} from "@tur/edgy";

render(() =>
    Container({
        height: 100,
        width: 200,
        padding: 20,
        children: [
            Row({
                crossAlignment: CrossAxisAlignment.Start,
                children: [SizedBox({ width: 40, height: 40 })],
            }),
        ],
    }),
);
