import { transform } from "sucrase";

let runtimeCode: string | null = null;

export async function initCompiler(): Promise<void> {
    const resp = await fetch("/runtime.js");
    if (!resp.ok) throw new Error(`Failed to load runtime: ${resp.status}`);
    runtimeCode = await resp.text();
}

export function compile(source: string): { code?: string; error?: string } {
    if (!runtimeCode) return { error: "Compiler not initialized" };

    try {
        const { code: jsCode } = transform(source, {
            transforms: ["typescript", "jsx"],
            jsxRuntime: "classic",
        });

        let result = jsCode;

        result = result.replace(
            /import\s+type\s+\{[\s\S]*?\}\s*from\s*["'][^"']*["']\s*;?/g,
            "",
        );

        result = result.replace(
            /import\s+(\w+)\s*,\s*\{([\s\S]*?)\}\s*from\s*["']react["']\s*;?/g,
            (_, name, imports) =>
                `var ${name} = globalThis.React;\nvar {${imports.trim()}} = globalThis.React;`,
        );

        result = result.replace(
            /import\s*\*\s*as\s+(\w+)\s+from\s*["']react["']\s*;?/g,
            (_, name) => `var ${name} = globalThis.React;`,
        );

        result = result.replace(
            /import\s+(\w+)\s+from\s*["']react["']\s*;?/g,
            (_, name) => `var ${name} = globalThis.React;`,
        );

        result = result.replace(
            /import\s*\{([\s\S]*?)\}\s*from\s*["']react["']\s*;?/g,
            (_, imports) => `var {${imports.trim()}} = globalThis.React;`,
        );

        result = result.replace(
            /import\s*\{([\s\S]*?)\}\s*from\s*["']@tur\/react["']\s*;?/g,
            (_, imports) => `var {${imports.trim()}} = globalThis.TurReact;`,
        );

        result = result.replace(
            /import\s*\{([\s\S]*?)\}\s*from\s*["']@tur\/react-renderer["']\s*;?/g,
            (_, imports) =>
                `var {${imports.trim()}} = globalThis.TurReactRenderer;`,
        );

        result = result.replace(
            /import\s*\{([\s\S]*?)\}\s*from\s*["']jotai\/vanilla["']\s*;?/g,
            (_, imports) =>
                `var {${imports.trim()}} = globalThis.JotaiVanilla;`,
        );

        result = result.replace(
            /import\s*\{([\s\S]*?)\}\s*from\s*["']jotai\/react["']\s*;?/g,
            (_, imports) => `var {${imports.trim()}} = globalThis.JotaiReact;`,
        );

        const localImports = result.match(
            /import\s+[\s\S]*?from\s*["']\.{0,2}\/[^"']*["']\s*;?/g,
        );
        if (localImports) {
            return {
                error: `Local imports not supported for live editing: ${localImports[0].split("\n")[0]}`,
            };
        }

        const remainingImports = result.match(
            /import\s+[\s\S]*?from\s*["'][^"']*["']\s*;?/g,
        );
        if (remainingImports) {
            return {
                error: `Unsupported import: ${remainingImports[0].split("\n")[0]}`,
            };
        }

        if (
            !result.includes("globalThis.React") &&
            /\bReact\.createElement\b/.test(result)
        ) {
            result = `var React = globalThis.React;\n${result}`;
        }

        return { code: `${runtimeCode}\n${result}` };
    } catch (e) {
        return { error: e instanceof Error ? e.message : String(e) };
    }
}
