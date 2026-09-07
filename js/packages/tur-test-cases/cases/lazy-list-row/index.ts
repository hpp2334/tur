import { Axis, Color, Container, LazyList, mount, view } from "tur:std";

const ITEM_WIDTH = 80;
const ITEM_COUNT = 50;

const App = view(() =>
    LazyList({ itemCount: ITEM_COUNT })
        .axis(Axis.Horizontal)
        .overscan(0)
        .queryKey(["lazy-list-row"])
        .builder((i: number) =>
            Container()
                .width(ITEM_WIDTH)
                .height(300)
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
