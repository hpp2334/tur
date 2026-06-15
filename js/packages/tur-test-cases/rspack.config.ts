import { globSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@rspack/cli";

const casesDir = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "edgy-cases",
);
const entries: Record<string, string> = {};

for (const dir of globSync("*/index.ts", { cwd: casesDir })) {
    const name = dir.split("/")[0];
    entries[name] = `./edgy-cases/${dir}`;
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
