import { Axis, Color, Container, LazyGrid, mount, Text, view } from "tur:std";

// Virtualized `LazyGrid`: 500 items, only the cells in the viewport + overscan
// are mounted. Wheel-scroll to reveal more. Cells are uniform square tiles
// (default aspect), labeled with their index.
const ITEM_COUNT = 500;

function hueFor(i: number): number {
    return (i * 47) % 360;
}

function hslToHex(h: number, s: number, l: number): string {
    const sat = s / 100;
    const light = l / 100;
    const c = (1 - Math.abs(2 * light - 1)) * sat;
    const hp = h / 60;
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
    const m = light - c / 2;
    const r = Math.round((r1 + m) * 255);
    const g = Math.round((g1 + m) * 255);
    const b = Math.round((b1 + m) * 255);
    return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

const App = view(() =>
    LazyGrid({
        axis: Axis.Vertical,
        itemCount: ITEM_COUNT,
        maxCrossAxisExtent: 110,
        crossAxisSpacing: 4,
        mainAxisSpacing: 4,
        overscan: 1,
        queryKey: ["lazy-grid-basic"],
        builder: (i: number) =>
            Container({
                color: Color.hex(hslToHex(hueFor(i), 55, 50)),
                children: [
                    Text({
                        text: `${i}`,
                        fontSize: 11,
                        color: Color.hex("#ffffff"),
                    }),
                ],
            }),
    }),
);

export function start() {
    mount(App);
}
