import { Color } from "@tur/edgy";

/** `__turHost` is registered by tur-wasm (swc-backed compiler services). */
interface TurHost {
    transpileTsx(src: string): string;
    tokenizeTsx(src: string): TokenSpan[];
}
interface TokenSpan {
    start: number;
    end: number;
    kind: number;
}

const host = (): TurHost =>
    (globalThis as unknown as { __turHost: TurHost }).__turHost;

/** Highlight palette indexed by token kind (see tur-wasm `classify_token`). */
const KIND_HEX = [
    "#abb2bf", // 0 default
    "#c678dd", // 1 keyword
    "#98c379", // 2 string
    "#d19a66", // 3 number
    "#7f848e", // 4 comment
    "#56b6c2", // 5 operator/punct
    "#e5c07b", // 6 literal (true/false/null)
];

export interface CaseCompileResult {
    error?: string;
    /** A component factory (the case's default export). */
    component?: () => unknown;
}

/**
 * Transpile a case source (TSX → JS via the swc bridge), rewrite its
 * `@tur/edgy` import to the in-scope `globalThis.TurEdgy`, capture the
 * `export default`, and eval it. Returns the case's component factory.
 */
export function compileCase(source: string): CaseCompileResult {
    let js: string;
    try {
        js = host().transpileTsx(source);
    } catch (e) {
        return {
            error: `transpile: ${e instanceof Error ? e.message : String(e)}`,
        };
    }

    // `import { A, B } from "@tur/edgy"` → `const { A, B } = globalThis.TurEdgy`
    js = js.replace(
        /import\s*\{([\s\S]*?)\}\s*from\s*["']@tur\/edgy["'];?/g,
        (_m, specs: string) => `const {${specs}} = globalThis.TurEdgy;`,
    );
    // Local relative imports aren't supported (single-file MVP).
    js = js.replace(/import\s+[^;]*from\s*["'][./][^"']*["'];?/g, "");
    // `export default <expr>` → `globalThis.__tur_case = <expr>`
    js = js.replace(/export\s+default\s+/g, "globalThis.__tur_case = ");
    // Strip any remaining named exports.
    js = js.replace(
        /export\s+(?:const|let|var|function|class)\s/g,
        "var __unused_ = ",
    );
    js = js.replace(/export\s*\{[^}]*\}\s*;?/g, "");

    const g = globalThis as unknown as {
        __tur_case?: unknown;
        eval: (s: string) => void;
    };
    g.__tur_case = undefined;
    try {
        // Indirect eval (global scope). The local binding avoids the comma
        // operator and keeps biome's noCommaOperator rule happy.
        const evalInGlobal = g.eval;
        evalInGlobal(js);
    } catch (e) {
        return { error: `eval: ${e instanceof Error ? e.message : String(e)}` };
    }

    const comp = g.__tur_case;
    if (typeof comp !== "function") {
        return { error: "case has no default export component" };
    }
    return { component: comp as () => unknown };
}

/** Build colored `SpanData[]` for a source string by tokenizing it. */
export function buildHighlightSpans(
    source: string,
): Array<{ content: string; color?: unknown }> {
    let tokens: TokenSpan[] = [];
    try {
        tokens = host().tokenizeTsx(source);
    } catch {
        return [{ content: source, color: Color.hex(KIND_HEX[0]) }];
    }

    const spans: Array<{ content: string; color?: unknown }> = [];
    let pos = 0;
    for (const t of tokens) {
        if (t.start > pos) {
            spans.push({
                content: source.slice(pos, t.start),
                color: Color.hex(KIND_HEX[0]),
            });
        }
        spans.push({
            content: source.slice(t.start, t.end),
            color: Color.hex(KIND_HEX[t.kind] ?? KIND_HEX[0]),
        });
        pos = t.end;
    }
    if (pos < source.length) {
        spans.push({
            content: source.slice(pos),
            color: Color.hex(KIND_HEX[0]),
        });
    }
    return spans;
}
