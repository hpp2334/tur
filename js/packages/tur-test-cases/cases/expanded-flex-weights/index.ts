import {
    Column,
    CrossAxisAlignment,
    Expanded,
    mount,
    SizedBox,
    view,
} from "tur:std";

const App = view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            Expanded({ flex: 2, child: SizedBox({}) }),
            Expanded({ flex: 1, child: SizedBox({}) }),
        ],
    }),
);

export function start() {
    mount(App);
}
