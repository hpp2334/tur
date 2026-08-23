import {
    Column,
    CrossAxisAlignment,
    MainAxisSize,
    mount,
    Row,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
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

export function start() {
    mount(App);
}
