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

/** Regex that matches `import { ... } from "./foo"` or `from "./foo.ts"`. */
const RELATIVE_IMPORT_RE =
    /import\s*(?:\{([\s\S]*?)\}|(\w+))\s*from\s*["']\.\/([^"']+)["'];?/g;

/**
 * Compile a case from its file map. Handles both single-file and multi-file
 * cases.
 *
 * - Single-file: `{ "index.ts": "..." }` — compiled as before.
 * - Multi-file: `{ "index.ts": "...", "utils.ts": "..." }` — non-entry files
 *   are registered as virtual modules on `globalThis.__tur_modules`, then the
 *   entry file is eval'd.
 */
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

    // Reset module registry for this compilation.
    g.__tur_modules = {};
    g.__tur_case = undefined;

    // Process non-entry files first (register as virtual modules).
    const nonEntryFiles = Object.entries(files).filter(
        ([name]) => name !== "index.ts",
    );
    for (const [filename, src] of nonEntryFiles) {
        let js: string;
        try {
            js = host().transpileTsx(src);
        } catch (e) {
            return {
                error: `transpile ${filename}: ${e instanceof Error ? e.message : String(e)}`,
            };
        }

        // Rewrite @tur/edgy imports.
        js = js.replace(
            /import\s*\{([\s\S]*?)\}\s*from\s*["']@tur\/edgy["'];?/g,
            (_m, specs: string) => `const {${specs}} = globalThis.TurEdgy;`,
        );

        // Rewrite relative imports.
        js = js.replace(
            RELATIVE_IMPORT_RE,
            (_m, namedSpecs: string, defaultSpec: string, modulePath: string) => {
                const moduleName = modulePath.replace(/\.ts$/, "");
                if (namedSpecs) {
                    return `const {${namedSpecs}} = globalThis.__tur_modules["${moduleName}"];`;
                }
                if (defaultSpec) {
                    return `const ${defaultSpec} = globalThis.__tur_modules["${moduleName}"];`;
                }
                return "";
            },
        );

        // Rewrite exports to build an exports object.
        // `export const X = ...` → `var X = ...` + `exports.X = X` at end
        // Simple approach: strip export keyword, track exported names, assign at end.
        const exportedNames: string[] = [];
        js = js.replace(
            /export\s+(?:const|let|var)\s+(\w+)/g,
            (_m, name: string) => {
                exportedNames.push(name);
                return `var ${name}`;
            },
        );
        js = js.replace(
            /export\s+function\s+(\w+)/g,
            (_m, name: string) => {
                exportedNames.push(name);
                return `function ${name}`;
            },
        );
        js = js.replace(
            /export\s+class\s+(\w+)/g,
            (_m, name: string) => {
                exportedNames.push(name);
                return `var ${name} = class ${name}`;
            },
        );
        // Strip re-exports like `export { X, Y }`.
        js = js.replace(/export\s*\{([^}]*)\}\s*;?/g, (_m, names: string) => {
            for (const n of names.split(",")) {
                const trimmed = n.trim().split(/\s+as\s+/)[0]?.trim();
                if (trimmed) exportedNames.push(trimmed);
            }
            return "";
        });

        // Handle `export default` — assign to exports.default.
        js = js.replace(/export\s+default\s+/g, "exports.default = ");

        const moduleName = filename.replace(/\.ts$/, "");
        const wrappedJs = [
            `globalThis.__tur_modules["${moduleName}"] = (function() {`,
            `var exports = {};`,
            js,
            ...exportedNames.map((n) => `exports.${n} = ${n};`),
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
    let entryJs: string;
    try {
        entryJs = host().transpileTsx(entryFile);
    } catch (e) {
        return {
            error: `transpile index.ts: ${e instanceof Error ? e.message : String(e)}`,
        };
    }

    // Rewrite @tur/edgy imports.
    entryJs = entryJs.replace(
        /import\s*\{([\s\S]*?)\}\s*from\s*["']@tur\/edgy["'];?/g,
        (_m, specs: string) => `const {${specs}} = globalThis.TurEdgy;`,
    );

    // Rewrite relative imports.
    entryJs = entryJs.replace(
        RELATIVE_IMPORT_RE,
        (_m, namedSpecs: string, defaultSpec: string, modulePath: string) => {
            const moduleName = modulePath.replace(/\.ts$/, "");
            if (namedSpecs) {
                return `const {${namedSpecs}} = globalThis.__tur_modules["${moduleName}"];`;
            }
            if (defaultSpec) {
                return `const ${defaultSpec} = globalThis.__tur_modules["${moduleName}"];`;
            }
            return "";
        },
    );

    // `export default <expr>` → `globalThis.__tur_case = <expr>`
    entryJs = entryJs.replace(
        /export\s+default\s+/g,
        "globalThis.__tur_case = ",
    );

    // Strip remaining named exports.
    entryJs = entryJs.replace(
        /export\s+(?:const|let|var|function|class)\s/g,
        "var __unused_export_ = ",
    );
    entryJs = entryJs.replace(/export\s*\{[^}]*\}\s*;?/g, "");

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
