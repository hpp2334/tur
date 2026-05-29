import type { ImportDeclaration, ImportSpecifier } from "acorn";
import { parse } from "acorn";
import { transform } from "sucrase";

let runtimeCode: string | null = null;

export async function initCompiler(): Promise<void> {
    const resp = await fetch("/runtime.js");
    if (!resp.ok) throw new Error(`Failed to load runtime: ${resp.status}`);
    runtimeCode = await resp.text();
}

const GLOBAL_MAP: Record<string, string> = {
    "@tur/react": "globalThis.TurReact",
    "@tur/react-renderer": "globalThis.TurReactRenderer",
    react: "globalThis.React",
    jotai: "globalThis.Jotai",
    "jotai/vanilla": "globalThis.JotaiVanilla",
    "jotai/react": "globalThis.JotaiReact",
};

function formatSpecifier(spec: ImportSpecifier): string {
    const imported =
        spec.imported.type === "Identifier"
            ? spec.imported.name
            : spec.imported.value;
    if (imported === spec.local.name) return imported;
    return `${imported}: ${spec.local.name}`;
}

function processImport(node: ImportDeclaration): string | null {
    const source = node.source.value as string;

    if (source.startsWith(".")) {
        return null;
    }

    const globalRef = GLOBAL_MAP[source];
    if (!globalRef) {
        return undefined as unknown as string;
    }

    const lines: string[] = [];

    for (const spec of node.specifiers) {
        if (spec.type === "ImportDefaultSpecifier") {
            lines.push(`var ${spec.local.name} = ${globalRef};`);
        } else if (spec.type === "ImportNamespaceSpecifier") {
            lines.push(`var ${spec.local.name} = ${globalRef};`);
        } else if (spec.type === "ImportSpecifier") {
            lines.push(`var { ${formatSpecifier(spec)} } = ${globalRef};`);
        }
    }

    return lines.join("\n");
}

function resolveLocalPath(
    importer: string,
    specifier: string,
    files: Map<string, string>,
): string | null {
    const match = specifier.match(/^\.\/(.+)$/);
    if (!match) return null;
    let base = match[1];
    if (base.endsWith(".js")) {
        base = base.slice(0, -3);
    }
    if (/\.(ts|tsx)$/.test(base)) {
        if (files.has(base)) return base;
        return null;
    }
    const dir = importer.includes("/")
        ? importer.substring(0, importer.lastIndexOf("/"))
        : "";
    for (const ext of [".tsx", ".ts"]) {
        const candidate = dir ? `${dir}/${base}${ext}` : `${base}${ext}`;
        if (files.has(candidate)) return candidate;
    }
    const bare = dir ? `${dir}/${base}` : base;
    if (files.has(bare)) return bare;
    return null;
}

function transpileFile(
    fileName: string,
    source: string,
    files: Map<string, string>,
    visited: Set<string>,
    output: string[],
): { code?: string; error?: string } {
    if (visited.has(fileName)) return {};
    visited.add(fileName);

    let jsCode: string;
    try {
        const result = transform(source, {
            transforms: ["typescript", "jsx"],
            jsxRuntime: "classic",
        });
        jsCode = result.code;
    } catch (e) {
        return {
            error: `${fileName}: ${e instanceof Error ? e.message : String(e)}`,
        };
    }

    let ast: ReturnType<typeof parse>;
    try {
        ast = parse(jsCode, {
            sourceType: "module",
            ecmaVersion: 2024,
        });
    } catch (e) {
        return {
            error: `${fileName}: parse error: ${e instanceof Error ? e.message : String(e)}`,
        };
    }

    const imports = ast.body.filter(
        (node): node is ImportDeclaration => node.type === "ImportDeclaration",
    );

    const exports = ast.body.filter(
        (node) =>
            node.type === "ExportNamedDeclaration" ||
            node.type === "ExportDefaultDeclaration",
    );

    const replacements: { start: number; end: number; text: string }[] = [];

    for (const exp of exports) {
        if (exp.type === "ExportNamedDeclaration" && exp.declaration) {
            replacements.push({
                start: exp.start,
                end: exp.declaration.start,
                text: "",
            });
        } else if (exp.type === "ExportDefaultDeclaration") {
            replacements.push({
                start: exp.start,
                end: exp.end,
                text: "",
            });
        } else if (
            exp.type === "ExportNamedDeclaration" &&
            exp.specifiers.length > 0
        ) {
            replacements.push({
                start: exp.start,
                end: exp.end,
                text: "",
            });
        }
    }

    for (const node of imports) {
        const src = node.source.value as string;

        if (src.startsWith(".")) {
            const resolved = resolveLocalPath(fileName, src, files);
            if (!resolved) {
                return {
                    error: `${fileName}: cannot resolve local import: ${src}`,
                };
            }
            const depSource = files.get(resolved);
            if (depSource === undefined) {
                return {
                    error: `${fileName}: local file not found: ${resolved}`,
                };
            }

            const depResult = transpileFile(
                resolved,
                depSource,
                files,
                visited,
                output,
            );
            if (depResult.error) return depResult;

            replacements.push({
                start: node.start,
                end: node.end,
                text: "",
            });
            continue;
        }

        const globalRef = GLOBAL_MAP[src];
        if (!globalRef) {
            return { error: `Unsupported import: ${src}` };
        }

        const result = processImport(node);
        replacements.push({
            start: node.start,
            end: node.end,
            text: result ?? "",
        });
    }

    let processed = jsCode;
    for (const { start, end, text } of replacements.sort(
        (a, b) => b.start - a.start,
    )) {
        processed = processed.slice(0, start) + text + processed.slice(end);
    }

    output.push(processed);
    return {};
}

export function compile(source: string): { code?: string; error?: string } {
    return compileWithFiles("index.tsx", source, new Map());
}

export function compileWithFiles(
    entryFile: string,
    source: string,
    files: Map<string, string>,
): { code?: string; error?: string } {
    if (!runtimeCode) return { error: "Compiler not initialized" };

    try {
        files.set(entryFile, source);

        const output: string[] = [];
        const visited = new Set<string>();
        const result = transpileFile(entryFile, source, files, visited, output);

        if (result.error) return result;

        const processed = output.join("\n");

        const hasReactRef = /var React\s*=/.test(processed);
        const finalCode = hasReactRef
            ? processed
            : `var React = globalThis.React;\n${processed}`;

        return { code: `${runtimeCode}\n${finalCode}` };
    } catch (e) {
        return { error: e instanceof Error ? e.message : String(e) };
    }
}
