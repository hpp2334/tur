import {
    Column,
    CrossAxisAlignment,
    view,
    MainAxisSize,
    SizedBox,
} from "@tur/edgy";

export default view(() =>
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
