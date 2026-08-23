import { Axis, Color, Container, LazyGrid, mount, view } from "tur:std";

// Large virtualized `LazyGrid`: 5000 items with a fixed `mainAxisExtent`
// (60px row height) + `childAspectRatio`. Wheel-scroll vertically; only the
// visible rows mount. A dark zebra background makes the tiling + scroll
// bounds easy to see.
const ITEM_COUNT = 5000;

function cellColor(i: number) {
    const hue = (i * 37) % 360;
    const sat = 50;
    const light = 45;
    const s = sat / 100;
    const l = light / 100;
    const c = (1 - Math.abs(2 * l - 1)) * s;
    const hp = hue / 60;
    const x = c * (1 - Math.abs((hp % 2) - 1));
    let r1 = 0,
        g1 = 0,
        b1 = 0;
    if (hp < 1) {
        r1 = c;
        g1 = x;
    } else if (hp < 2) {
        r1 = x;
        g1 = c;
    } else if (hp < 3) {
        g1 = c;
        b1 = x;
    } else if (hp < 4) {
        g1 = x;
        b1 = c;
    } else if (hp < 5) {
        r1 = x;
        b1 = c;
    } else {
        r1 = c;
        b1 = x;
    }
    const m = l - c / 2;
    const r = Math.round((r1 + m) * 255);
    const g = Math.round((g1 + m) * 255);
    const b = Math.round((b1 + m) * 255);
    return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

const App = view(() =>
    LazyGrid({
        axis: Axis.Vertical,
        itemCount: ITEM_COUNT,
        maxCrossAxisExtent: 120,
        mainAxisExtent: 60,
        crossAxisSpacing: 6,
        mainAxisSpacing: 6,
        overscan: 2,
        queryKey: ["lazy-grid-scroll"],
        builder: (i: number) =>
            Container({
                color: Color.hex(cellColor(i)),
            }),
    }),
);

export function start() {
    mount(App);
}
