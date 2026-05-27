import { parse } from "acorn";
import type { ImportDeclaration, ImportSpecifier } from "acorn";
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
    const imported = spec.imported.type === "Identifier" ? spec.imported.name : spec.imported.value;
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

export function compile(source: string): { code?: string; error?: string } {
    if (!runtimeCode) return { error: "Compiler not initialized" };

    try {
        const { code: jsCode } = transform(source, {
            transforms: ["typescript", "jsx"],
            jsxRuntime: "classic",
        });

        const ast = parse(jsCode, {
            sourceType: "module",
            ecmaVersion: 2024,
        });

        const imports = ast.body.filter(
            (node): node is ImportDeclaration => node.type === "ImportDeclaration",
        );

        const replacements: { start: number; end: number; text: string }[] = [];
        let hasReactRef = false;

        for (const node of imports) {
            const source = node.source.value as string;

            if (source.startsWith(".")) {
                return {
                    error: `Local imports not supported for live editing: ${source}`,
                };
            }

            const globalRef = GLOBAL_MAP[source];
            if (!globalRef) {
                return { error: `Unsupported import: ${source}` };
            }

            hasReactRef = hasReactRef || source === "react";

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
            processed =
                processed.slice(0, start) + text + processed.slice(end);
        }

        if (
            !hasReactRef &&
            /\bReact\.createElement\b/.test(processed)
        ) {
            processed = `var React = globalThis.React;\n${processed}`;
        }

        return { code: `${runtimeCode}\n${processed}` };
    } catch (e) {
        return { error: e instanceof Error ? e.message : String(e) };
    }
}
