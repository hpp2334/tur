import {
    Color,
    Column,
    Container,
    Expanded,
    Text,
    view,
} from "builtin:tur/std";
import { tokenizeTsx, transpileTsx } from "builtin:tur/host";

const SRC = "const x: number = 42;";
const OUT = transpileTsx(SRC);

// Build colored spans from the tokenizer output.
const HL = "const y = 1;";
const KIND_HEX = [
    "#abb2bf",
    "#c678dd",
    "#98c379",
    "#d19a66",
    "#7f848e",
    "#56b6c2",
    "#e5c07b",
];
const spans: Array<{ content: string; color?: unknown }> = [];
let pos = 0;
for (const t of tokenizeTsx(HL)) {
    if (t.start > pos) {
        spans.push({ content: HL.slice(pos, t.start) });
    }
    spans.push({
        content: HL.slice(t.start, t.end),
        color: Color.hex(KIND_HEX[t.kind] ?? "#abb2bf"),
    });
    pos = t.end;
}
if (pos < HL.length) {
    spans.push({ content: HL.slice(pos) });
}

export default view(() =>
    Expanded({
        child: Container({
            color: Color.hex("#282c34"),
            padding: 20,
            children: [
                Column({
                    children: [
                        Text({
                            text: "transpileTsx (via builtin:tur/host):",
                            fontSize: 13,
                            color: Color.hex("#848da5"),
                        }),
                        Container({ height: 6 }),
                        Text({
                            text: SRC,
                            fontSize: 16,
                            color: Color.hex("#e06c75"),
                        }),
                        Text({
                            text: `→ ${OUT}`,
                            fontSize: 16,
                            color: Color.hex("#98c379"),
                        }),
                        Container({ height: 18 }),
                        Text({
                            text: "tokenizeTsx highlight:",
                            fontSize: 13,
                            color: Color.hex("#848da5"),
                        }),
                        Container({ height: 6 }),
                        Text({ fontSize: 20, spans } as never),
                    ],
                }),
            ],
        }),
    }),
);
