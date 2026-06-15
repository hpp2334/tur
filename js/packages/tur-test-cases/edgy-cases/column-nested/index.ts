import {
    Column,
    CrossAxisAlignment,
    MainAxisSize,
    SizedBox,
    render,
} from "@tur/edgy";

render(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            SizedBox({ height: 50 }),
            Column({
                crossAlignment: CrossAxisAlignment.Start,
                mainAxisSize: MainAxisSize.Min,
                children: [SizedBox({ height: 30 })],
            }),
            SizedBox({ height: 40 }),
        ],
    }),
);
