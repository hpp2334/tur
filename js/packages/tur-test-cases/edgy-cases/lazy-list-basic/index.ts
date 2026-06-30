import { Axis, Color, Container, view, LazyList } from "@tur/edgy";

const ITEM_HEIGHT = 50;
const ITEM_COUNT = 20;

export default view(() =>
    LazyList({
        axis: Axis.Vertical,
        itemCount: ITEM_COUNT,
        overscan: 0,
        queryKey: ["lazy-list-basic"],
        builder: (i: number) =>
            Container({
                width: 400,
                height: ITEM_HEIGHT,
                color:
                    i % 2 === 0 ? Color.rgb(48, 48, 48) : Color.rgb(26, 26, 26),
            }),
    }),
);
