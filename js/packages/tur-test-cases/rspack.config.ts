import {
    globSync,
    mkdirSync,
    readFileSync,
    rmSync,
    writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@rspack/cli";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const casesDir = path.resolve(__dirname, "edgy-cases");
const entriesDir = path.resolve(__dirname, ".test-entries");

// Each case source `export default`s a component and no longer calls
// `render()`. The legacy playground and the Rust integration tests eval
// `dist/<name>.js` and expect the tree to mount on eval, so we emit a tiny
// wrapper per case that imports the default and calls `render()` on it.
rmSync(entriesDir, { recursive: true, force: true });
mkdirSync(entriesDir, { recursive: true });

const entries: Record<string, string> = {};
for (const dir of globSync("*/index.ts", { cwd: casesDir })) {
    const name = dir.split("/")[0];
    const wrapper = path.join(entriesDir, `${name}.ts`);
    writeFileSync(
        wrapper,
        `import Case from "../edgy-cases/${name}/index";\nimport { render } from "@tur/edgy";\nrender(Case);\n`,
    );
    entries[name] = `./.test-entries/${name}.ts`;
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
