import { Color, Column, Container, Expanded, Text, view } from "@tur/edgy";

// `__turHost` is registered by tur-wasm (swc-backed). tur-engine provides the
// generic `register_host_fn` hook; tur-wasm supplies the compiler impls.
const host = (
    globalThis as unknown as {
        __turHost: {
            transpileTsx(src: string): string;
            tokenizeTsx(src: string): Array<{
                start: number;
                end: number;
                kind: number;
            }>;
        };
    }
).__turHost;

const SRC = "const x: number = 42;";
const OUT = host.transpileTsx(SRC);

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
for (const t of host.tokenizeTsx(HL)) {
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
                            text: "transpileTsx (via __turHost bridge):",
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
