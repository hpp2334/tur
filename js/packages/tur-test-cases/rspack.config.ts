import { globSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@rspack/cli";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const casesDir = path.resolve(__dirname, "cases");

// Each case source authors the module lifecycle contract directly: an
// `export function start(...)` that calls `mount(view)` against the
// engine-provided instance store. The dist module IS the case source
// (transpiled, `tur:*` imports kept external) — no wrapper needed.
const entries: Record<string, string> = {};
for (const dir of globSync("*/index.ts", { cwd: casesDir })) {
    const name = dir.split("/")[0];
    entries[name] = `./cases/${name}/index.ts`;
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
    plugins: [],
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
