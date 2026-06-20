import { code } from "../theme/tokens";

/** `__turHost` is registered by tur-wasm (swc-backed compiler services). */
interface TurHost {
    transpileTsx(src: string): string;
    tokenizeTsx(src: string): TokenSpan[];
    generateAst(src: string): AstNode[];
}
interface TokenSpan {
    start: number;
    end: number;
    kind: number;
}

/** AST node returned by `generateAst`. Each node includes the exact source
 *  text (`text`) for that declaration, extracted by Rust — so no fragile
 *  position arithmetic on the JS side.  For export nodes, `body` contains
 *  the declaration text WITHOUT the `export`/`export default` keyword,
 *  also extracted by Rust from the inner declaration's span. */
interface AstNode {
    kind:
        | "import"
        | "exportDecl"
        | "exportDefault"
        | "exportNamed"
        | "exportAll"
        | "exportType"
        | "statement";
    text: string;
    /** For export nodes: text without the `export` keyword. */
    body?: string;
    source?: string;
    specifiers?: Array<{ local: string; imported: string }>;
    names?: string[];
}

const host = (): TurHost =>
    (globalThis as unknown as { __turHost: TurHost }).__turHost;

/** Highlight palette indexed by token kind (see tur-wasm `highlight_tsx`).
 *  0–6 are lexical (lexer); 7–11 are AST-derived semantic categories.
 *  Pulled from `code.*` design tokens — see DESIGN-SYSTEM.md §1.1. */
const KIND_COLOR: unknown[] = [
    code.fg, // 0 default
    code.keyword, // 1 keyword
    code.string, // 2 string
    code.number, // 3 number
    code.comment, // 4 comment
    code.operator, // 5 operator/punct
    code.literal, // 6 literal (true/false/null)
    code.decl, // 7 declaration / import / call-callee name
    code.jsxTag, // 8 JSX tag name
    code.jsxAttr, // 9 JSX attribute name
    code.type, // 10 interface/type name
    code.property, // 11 object-literal key / member `.prop`
];

export interface CaseCompileResult {
    error?: string;
    /** The case's default export — a component handle (`EdgyElement`). */
    component?: unknown;
}

// ---------------------------------------------------------------------------
// AST-based module rewriting
// ---------------------------------------------------------------------------

/** Build the destructuring names string from import specifiers.
 *  `[{local:"X", imported:"X"}]` → `"X"`
 *  `[{local:"Y", imported:"X"}]` → `"X: Y"` */
function specNames(specs: Array<{ local: string; imported: string }>): string {
    return specs
        .map((s) =>
            s.imported === s.local ? s.local : `${s.imported}: ${s.local}`,
        )
        .join(", ");
}

/** Normalize a relative import path to a module name key.
 *  `"./foo.ts"` → `"foo"`, `"./foo"` → `"foo"`.
 *  No regex — pure string manipulation. */
function moduleKey(source: string): string {
    let name = source;
    if (name.startsWith("./")) name = name.slice(2);
    if (name.endsWith(".ts")) name = name.slice(0, name.length - 3);
    return name;
}

/** Normalize a relative import path to a file name key.
 *  `"./foo"` → `"foo.ts"`, `"./foo.ts"` → `"foo.ts"`.
 *  No regex — pure string manipulation. */
function fileKey(source: string): string {
    let name = source;
    if (name.startsWith("./")) name = name.slice(2);
    if (!name.endsWith(".ts")) name = `${name}.ts`;
    return name;
}

/** Rewrite a transpiled JS string using AST metadata from `generateAst`.
 *  Returns the rewritten source and the list of exported names.
 *
 *  Each AST node carries its own `text` (full node text) and `body` (export
 *  nodes only — text without the `export` keyword), both extracted safely by
 *  Rust. No regex or position arithmetic on the JS side. */
function rewriteModule(transpiled: string): {
    source: string;
    exportedNames: string[];
} {
    const ast = host().generateAst(transpiled);
    const parts: string[] = [];
    const exportedNames: string[] = [];

    for (const node of ast) {
        switch (node.kind) {
            case "import": {
                const src = node.source ?? "";
                const names = specNames(node.specifiers ?? []);
                if (src === "@tur/edgy") {
                    parts.push(`const {${names}} = globalThis.TurEdgy;`);
                } else {
                    parts.push(
                        `const {${names}} = globalThis.__tur_modules["${moduleKey(src)}"];`,
                    );
                }
                break;
            }

            case "exportDecl": {
                parts.push(node.body ?? node.text);
                if (node.names) exportedNames.push(...node.names);
                break;
            }

            case "exportDefault": {
                parts.push(`exports.default = ${node.body ?? node.text};`);
                exportedNames.push("default");
                break;
            }

            case "exportNamed": {
                if (node.names) exportedNames.push(...node.names);
                break;
            }

            case "exportAll":
            case "exportType": {
                break;
            }

            case "statement": {
                parts.push(node.text);
                break;
            }
        }
    }

    return { source: parts.join("\n"), exportedNames };
}

/** Rewrite the entry file. Like `rewriteModule` but converts
 *  `export default X` → `globalThis.__tur_case = X` instead of
 *  `exports.default = X`, and drops other exports without tracking. */
function rewriteEntry(transpiled: string): string {
    const ast = host().generateAst(transpiled);
    const parts: string[] = [];

    for (const node of ast) {
        switch (node.kind) {
            case "import": {
                const src = node.source ?? "";
                const names = specNames(node.specifiers ?? []);
                if (src === "@tur/edgy") {
                    parts.push(`const {${names}} = globalThis.TurEdgy;`);
                } else {
                    parts.push(
                        `const {${names}} = globalThis.__tur_modules["${moduleKey(src)}"];`,
                    );
                }
                break;
            }

            case "exportDecl": {
                parts.push(node.body ?? node.text);
                break;
            }

            case "exportDefault": {
                parts.push(
                    `globalThis.__tur_case = ${node.body ?? node.text};`,
                );
                break;
            }

            case "exportNamed":
            case "exportAll":
            case "exportType": {
                break;
            }

            case "statement": {
                parts.push(node.text);
                break;
            }
        }
    }

    return parts.join("\n");
}

// ---------------------------------------------------------------------------
// Compile
// ---------------------------------------------------------------------------

export function compileCase(files: Record<string, string>): CaseCompileResult {
    const entryFile = files["index.ts"] ?? Object.values(files)[0];
    if (!entryFile) {
        return { error: "case has no files" };
    }

    const g = globalThis as unknown as {
        __tur_case?: unknown;
        __tur_modules?: Record<string, unknown>;
        eval: (s: string) => void;
    };
    const evalInGlobal = g.eval;

    g.__tur_modules = {};
    g.__tur_case = undefined;

    // Process non-entry files first, topologically sorted.
    const nonEntryFiles = Object.keys(files).filter(
        (name) => name !== "index.ts",
    );
    const sorted = topoSort(nonEntryFiles, files);
    for (const filename of sorted) {
        const src = files[filename];
        let transpiled: string;
        try {
            transpiled = host().transpileTsx(src);
        } catch (e) {
            return {
                error: `transpile ${filename}: ${e instanceof Error ? e.message : String(e)}`,
            };
        }

        let rewritten: { source: string; exportedNames: string[] };
        try {
            rewritten = rewriteModule(transpiled);
        } catch (e) {
            return {
                error: `rewrite ${filename}: ${e instanceof Error ? e.message : String(e)}`,
            };
        }

        const moduleName = moduleKey(filename);
        const wrappedJs = [
            `globalThis.__tur_modules["${moduleName}"] = (function() {`,
            `var exports = {};`,
            rewritten.source,
            ...rewritten.exportedNames.map((n) => `exports.${n} = ${n};`),
            `return exports;`,
            `})();`,
        ].join("\n");

        try {
            evalInGlobal(wrappedJs);
        } catch (e) {
            return {
                error: `eval ${filename}: ${e instanceof Error ? e.message : String(e)}`,
            };
        }
    }

    // Process entry file (index.ts).
    let transpiled: string;
    try {
        transpiled = host().transpileTsx(entryFile);
    } catch (e) {
        return {
            error: `transpile index.ts: ${e instanceof Error ? e.message : String(e)}`,
        };
    }

    let entryJs: string;
    try {
        entryJs = rewriteEntry(transpiled);
    } catch (e) {
        return {
            error: `rewrite index.ts: ${e instanceof Error ? e.message : String(e)}`,
        };
    }

    try {
        evalInGlobal(entryJs);
    } catch (e) {
        return {
            error: `eval index.ts: ${e instanceof Error ? e.message : String(e)}`,
        };
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
            spans.push({ content: source.slice(pos, t.start), color: code.fg });
        }
        const content = source.slice(t.start, t.end);
        // Skip zero-width tokens — an empty-content span would create an empty
        // parley style range (`start == end`) and crash text layout.
        if (content.length > 0) {
            spans.push({
                content,
                color: KIND_COLOR[t.kind] ?? KIND_COLOR[0],
            });
        }
        pos = t.end;
    }
    if (pos < source.length) {
        spans.push({ content: source.slice(pos), color: code.fg });
    }
    return spans;
}

/** Topologically sort file names so that dependencies come before dependents.
 *  Uses `generateAst` to find import sources — no regex. */
function topoSort(
    filenames: string[],
    allFiles: Record<string, string>,
): string[] {
    const deps = new Map<string, Set<string>>();
    for (const name of filenames) {
        const src = allFiles[name] ?? "";
        const d = new Set<string>();
        try {
            const ast = host().generateAst(src);
            for (const node of ast) {
                if (node.kind === "import" && node.source) {
                    const dep = fileKey(node.source);
                    if (dep !== name && filenames.includes(dep)) {
                        d.add(dep);
                    }
                }
            }
        } catch {
            // If AST parsing fails, treat as no dependencies.
        }
        deps.set(name, d);
    }
    const result: string[] = [];
    const done = new Set<string>();
    function visit(name: string): void {
        if (done.has(name)) return;
        done.add(name);
        for (const dep of deps.get(name) ?? []) {
            visit(dep);
        }
        result.push(name);
    }
    for (const name of [...filenames].sort()) {
        visit(name);
    }
    return result;
}
