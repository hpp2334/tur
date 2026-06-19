import { code } from "../theme/tokens";

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

/** Highlight palette indexed by token kind (see tur-wasm `classify_token`).
 *  Pulled from `code.*` design tokens — see DESIGN-SYSTEM.md §1.1. */
const KIND_COLOR: unknown[] = [
    code.fg, // 0 default
    code.keyword, // 1 keyword
    code.string, // 2 string
    code.number, // 3 number
    code.comment, // 4 comment
    code.operator, // 5 operator/punct
    code.literal, // 6 literal (true/false/null)
];

export interface CaseCompileResult {
    error?: string;
    /** The case's default export — a component handle (`EdgyElement`). */
    component?: unknown;
}

/**
 * Transpile a case source (TSX → JS via the swc bridge), rewrite its
 * `@tur/edgy` import to the in-scope `globalThis.TurEdgy`, capture the
 * `export default`, and eval it. Returns the case's component handle.
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
    if (comp == null) {
        return { error: "case has no default export component" };
    }
    return { component: comp };
}

/** Build colored `SpanData[]` for a source string by tokenizing it. */
export function buildHighlightSpans(
    source: string,
): Array<{ content: string; color?: unknown }> {
    let tokens: TokenSpan[] = [];
    try {
        tokens = host().tokenizeTsx(source);
    } catch {
        return [{ content: source, color: code.fg }];
    }

    const spans: Array<{ content: string; color?: unknown }> = [];
    let pos = 0;
    for (const t of tokens) {
        if (t.start > pos) {
            spans.push({
                content: source.slice(pos, t.start),
                color: code.fg,
            });
        }
        spans.push({
            content: source.slice(t.start, t.end),
            color: KIND_COLOR[t.kind] ?? KIND_COLOR[0],
        });
        pos = t.end;
    }
    if (pos < source.length) {
        spans.push({
            content: source.slice(pos),
            color: code.fg,
        });
    }
    return spans;
}
