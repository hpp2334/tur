import * as Anim from "tur:animation";
import * as Clipboard from "tur:clipboard";
import * as Net from "tur:net";
import * as FilePicker from "tur:filepicker";
import type { Color, SpanData } from "tur:std";
import * as Std from "tur:std";
import type { AstNode, TokenSpan } from "tur-ext/demo-helper";
import * as Host from "tur-ext/demo-helper";
import { code } from "../theme/tokens";

/** Highlight palette indexed by token kind (see tur-wasm `highlight_tsx`).
 *  0–6 are lexical (lexer); 7–11 are AST-derived semantic categories.
 *  Pulled from `code.*` design tokens — see DESIGN-SYSTEM.md §1.1. */
const KIND_COLOR: Color[] = [
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
    /** The case's default export — a view handle (`Element`). */
    view?: unknown;
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

/** Map an import source to the expression the rewritten code destructures
 *  from. `tur:*` resolve to the injected module namespace; relative
 *  imports resolve to the local `__modules` registry. */
function importTarget(source: string): string {
    switch (source) {
        case "tur:std":
            return "Std";
        case "tur:animation":
            return "Anim";
        case "tur-ext/demo-helper":
            return "Host";
        case "tur:net":
            return "Net";
        case "tur:clipboard":
            return "Clipboard";
        case "tur:filepicker":
            return "FilePicker";
        default:
            return `__modules["${moduleKey(source)}"]`;
    }
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
    const ast = Host.generateAst(transpiled);
    const parts: string[] = [];
    const exportedNames: string[] = [];

    for (const node of ast) {
        switch (node.kind) {
            case "import": {
                const src = node.source ?? "";
                const names = specNames(node.specifiers ?? []);
                parts.push(`const {${names}} = ${importTarget(src)};`);
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
 *  `export default X` → `return X` (the entry factory's return value is the
 *  case view), and drops other exports without tracking. */
function rewriteEntry(transpiled: string): string {
    const ast = Host.generateAst(transpiled);
    const parts: string[] = [];

    for (const node of ast) {
        switch (node.kind) {
            case "import": {
                const src = node.source ?? "";
                const names = specNames(node.specifiers ?? []);
                parts.push(`const {${names}} = ${importTarget(src)};`);
                break;
            }

            case "exportDecl": {
                parts.push(node.body ?? node.text);
                break;
            }

            case "exportDefault": {
                parts.push(`return ${node.body ?? node.text};`);
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

/** Evaluate rewritten case code in an isolated function scope with the
 *  `tur:*` modules and the per-case `__modules` registry injected as
 *  parameters (no `globalThis` pollution). Returns the function's value. */
function runCaseBody(body: string, modules: Record<string, unknown>): unknown {
    const fn = new Function(
        "Std",
        "Anim",
        "Host",
        "Net",
        "Clipboard",
        "FilePicker",
        "__modules",
        body,
    );
    return fn(Std, Anim, Host, Net, Clipboard, FilePicker, modules);
}

export function compileCase(files: Record<string, string>): CaseCompileResult {
    const entryFile = files["index.ts"] ?? Object.values(files)[0];
    if (!entryFile) {
        return { error: "case has no files" };
    }

    const modules: Record<string, unknown> = {};

    // Process non-entry files first, topologically sorted.
    const nonEntryFiles = Object.keys(files).filter(
        (name) => name !== "index.ts",
    );
    const sorted = topoSort(nonEntryFiles, files);
    for (const filename of sorted) {
        const src = files[filename];
        let transpiled: string;
        try {
            transpiled = Host.transpileTsx(src);
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

        const body = [
            "var exports = {};",
            rewritten.source,
            ...rewritten.exportedNames.map((n) => `exports.${n} = ${n};`),
            "return exports;",
        ].join("\n");

        try {
            modules[moduleKey(filename)] = runCaseBody(body, modules);
        } catch (e) {
            return {
                error: `eval ${filename}: ${e instanceof Error ? e.message : String(e)}`,
            };
        }
    }

    // Process entry file (index.ts).
    let transpiled: string;
    try {
        transpiled = Host.transpileTsx(entryFile);
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

    let view: unknown;
    try {
        view = runCaseBody(entryJs, modules);
    } catch (e) {
        return {
            error: `eval index.ts: ${e instanceof Error ? e.message : String(e)}`,
        };
    }

    if (view == null) {
        return { error: "case has no default export view" };
    }
    return { view };
}

/** Build colored `SpanData[]` for a source string by tokenizing it. */
export function buildHighlightSpans(source: string): SpanData[] {
    let tokens: TokenSpan[] = [];
    try {
        tokens = Host.tokenizeTsx(source);
    } catch {
        return [{ content: source, color: code.fg }];
    }

    const spans: SpanData[] = [];
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
            const ast = Host.generateAst(src);
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

// Silence unused-import warnings for Net, Clipboard and FilePicker
// (referenced only inside generated case bodies via the `runCaseBody`
// injection, not directly here).
void Net;
void Clipboard;
void FilePicker;
