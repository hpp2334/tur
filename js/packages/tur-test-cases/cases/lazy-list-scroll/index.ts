import { Axis, Color, Container, LazyList, mount, view } from "tur:std";

const ITEM_HEIGHT = 50;
const ITEM_COUNT = 100;

const App = view(() =>
    LazyList({ itemCount: ITEM_COUNT })
        .axis(Axis.Vertical)
        .overscan(0)
        .queryKey(["lazy-list-scroll"])
        .builder((i: number) =>
            Container()
                .width(400)
                .height(ITEM_HEIGHT)
                .color(
                    i % 2 === 0 ? Color.rgb(48, 48, 48) : Color.rgb(26, 26, 26),
                )
                .build(),
        )
        .build(),
);

export function start() {
    mount(App);
}
