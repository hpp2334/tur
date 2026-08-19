import { Color, Container, createStore, Grid, view } from "tur:std";

export const store = createStore();

// Static `Grid`: 12 cells tiled row-major. The column count is derived from
// `maxCrossAxisExtent` (140) and the available width — e.g. a 800px viewport
// → floor(800 / 140) = 5 columns, cells ~ (800 − 4·8) / 5 ≈ 153px wide.
// Default square cells (no childAspectRatio / mainAxisExtent).
const COUNT = 12;

function hueFor(i: number): number {
    return (i * 360) / COUNT;
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
        maxCrossAxisExtent: 140,
        crossAxisSpacing: 8,
        mainAxisSpacing: 8,
        queryKey: ["grid-basic"],
        children: Array.from({ length: COUNT }, (_, i) =>
            Container({
                color: Color.hex(hslToHex(hueFor(i), 65, 55)),
            }),
        ),
    }),
);
