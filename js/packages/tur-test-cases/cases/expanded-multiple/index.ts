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
            Expanded({ child: SizedBox({}) }),
            Expanded({ child: SizedBox({}) }),
        ],
    }),
);

export function start() {
    mount(App);
}
