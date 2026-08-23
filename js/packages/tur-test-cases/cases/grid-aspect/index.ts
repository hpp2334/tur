import { Color, Container, Grid, Text, view } from "tur:std";

// `Grid` with `childAspectRatio: 2` → each cell is twice as wide as it is
// tall (cell_main = cell_cross / 2). Also exercises `mainAxisExtent` on the
// last few cells via a second grid below. 10 cells per grid.
const COUNT = 10;

function hueFor(i: number): number {
    return (i * 360) / COUNT + 40;
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

export default view(() =>
    Grid({
        maxCrossAxisExtent: 160,
        childAspectRatio: 2,
        crossAxisSpacing: 6,
        mainAxisSpacing: 6,
        queryKey: ["grid-aspect"],
        children: Array.from({ length: COUNT }, (_, i) =>
            Container({
                color: Color.hex(hslToHex(hueFor(i), 60, 55)),
                children: [
                    Text({
                        text: `#${i}`,
                        fontSize: 12,
                        color: Color.hex("#ffffff"),
                    }),
                ],
            }),
        ),
    }),
);
