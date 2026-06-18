import {
    Column,
    CrossAxisAlignment,
    component,
    MainAxisSize,
    Row,
    SizedBox,
} from "@tur/edgy";

export default component(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Row({
                crossAlignment: CrossAxisAlignment.Center,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    SizedBox({ width: 20, height: 20 }),
                    SizedBox({ width: 40, height: 10 }),
                ],
            }),
            SizedBox({ height: 30 }),
            SizedBox({ height: 20 }),
        ],
    }),
);
