import { globSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@rspack/cli";
import * as rspack from "@rspack/core";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const casesDir = path.resolve(__dirname, "cases");

// Each case source `export default`s a view; it never calls `mount()` itself.
// The Rust integration tests eval `dist/<name>.js` which must satisfy the
// module lifecycle contract (a `start` export), so the in-memory wrapper
// (rspack's VirtualModulesPlugin) imports the case's default export and
// mounts it inside `start({ store })` against the engine-provided instance
// store (also stashed on globalThis for case helper seams) — no generated
// files on disk.
const entries: Record<string, string> = {};
const virtualModules: Record<string, string> = {};
for (const dir of globSync("*/index.ts", { cwd: casesDir })) {
    const name = dir.split("/")[0];
    virtualModules[`virtual-entries/${name}.ts`] =
        `import Case from "../cases/${name}/index";\nimport { mount } from "tur:std";\nexport function start({ store }) {\n    globalThis.__store = store;\n    mount(Case);\n}\n`;
    entries[name] = `./virtual-entries/${name}.ts`;
}

export default defineConfig({
    mode: "production",
    optimization: {
        minimize: false,
    },
    experiments: { outputModule: true },
    entry: entries,
    output: {
        filename: "[name].js",
        library: {
            type: "module",
        },
        clean: true,
    },
    // `tur:*` capability modules are provided at run time by the
    // engine's boa module loader, so keep the imports rather than bundling
    // them. Each case dist is an ES module loaded via `load_module`.
    externals: {
        "tur:std": "tur:std",
        "tur:animation": "tur:animation",
        "tur:clipboard": "tur:clipboard",
        "tur:net": "tur:net",
    },
    plugins: [new rspack.experiments.VirtualModulesPlugin(virtualModules)],
    module: {
        rules: [
            {
                test: /\.tsx?$/,
                exclude: /node_modules/,
                use: {
                    loader: "builtin:swc-loader",
                    options: {
                        jsc: {
                            parser: {
                                syntax: "typescript",
                                tsx: false,
                            },
                        },
                    },
                },
            },
        ],
    },
    resolve: {
        extensions: [".ts", ".tsx", ".js"],
    },
});
