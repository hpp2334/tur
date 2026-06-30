import {
    Container,
    CrossAxisAlignment,
    view,
    Row,
    SizedBox,
} from "@tur/edgy";

export default view(() =>
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
