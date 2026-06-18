import { globSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@rspack/cli";
import * as rspack from "@rspack/core";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const casesDir = path.resolve(__dirname, "edgy-cases");

// Each case source `export default`s a component and no longer calls
// `render()`. The Rust integration tests eval `dist/<name>.js` and expect the
// tree to mount on eval, so we register an in-memory wrapper module per case
// (rspack's VirtualModulesPlugin) that imports the default export and renders
// it — no generated files on disk.
const entries: Record<string, string> = {};
const virtualModules: Record<string, string> = {};
for (const dir of globSync("*/index.ts", { cwd: casesDir })) {
    const name = dir.split("/")[0];
    virtualModules[`virtual-entries/${name}.ts`] =
        `import Case from "../edgy-cases/${name}/index";\nimport { render } from "@tur/edgy";\nrender(Case);\n`;
    entries[name] = `./virtual-entries/${name}.ts`;
}

export default defineConfig({
    mode: "production",
    optimization: {
        minimize: false,
    },
    entry: entries,
    output: {
        filename: "[name].js",
        library: {
            type: "iife",
        },
        clean: true,
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
