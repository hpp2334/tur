import {
    Column,
    CrossAxisAlignment,
    MainAxisSize,
    mount,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
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

export function start() {
    mount(App);
}
