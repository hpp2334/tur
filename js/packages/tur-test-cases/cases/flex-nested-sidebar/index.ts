import {
    Container as C2,
    Column,
    Container,
    CrossAxisAlignment,
    createStore,
    Row,
    SizedBox,
    view,
} from "tur:std";

export const store = createStore();

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
