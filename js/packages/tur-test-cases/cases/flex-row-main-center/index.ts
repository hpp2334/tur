import {
    CrossAxisAlignment,
    MainAxisAlignment,
    mount,
    Row,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    Row({
        mainAlignment: MainAxisAlignment.Center,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);

export function start() {
    mount(App);
}
