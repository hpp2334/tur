import {
    Container as C2,
    Column,
    Container,
    CrossAxisAlignment,
    mount,
    Row,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
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

export function start() {
    mount(App);
}
