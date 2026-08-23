import { Axis, Color, Container, LazyList, mount, view } from "tur:std";

const ITEM_WIDTH = 80;
const ITEM_COUNT = 50;

const App = view(() =>
    LazyList({
        axis: Axis.Horizontal,
        itemCount: ITEM_COUNT,
        overscan: 0,
        queryKey: ["lazy-list-row"],
        builder: (i: number) =>
            Container({
                width: ITEM_WIDTH,
                height: 300,
                color:
                    i % 2 === 0 ? Color.rgb(48, 48, 48) : Color.rgb(26, 26, 26),
            }),
    }),
);

export function start() {
    mount(App);
}
