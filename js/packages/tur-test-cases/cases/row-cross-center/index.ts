import {
    Container,
    CrossAxisAlignment,
    createStore,
    Row,
    SizedBox,
    view,
} from "tur:std";

export const store = createStore();

export default view(() =>
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
