import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@rspack/cli";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Builds the self-hosted playground as a single IIFE bundle (`impl.js`) that
// the thin `tur-demo` wrapper loads via `TurWasmApp.load_and_run_js`. The
// bundle contains @tur/edgy + the Shell UI + inlined case sources; it sets
// `globalThis.TurEdgy` and calls `render(Shell)` on eval.
export default defineConfig({
    entry: {
        impl: "./src/index.ts",
    },
    output: {
        path: resolve(__dirname, "dist"),
        filename: "impl.js",
        clean: true,
        library: { type: "iife" },
    },
    optimization: {
        minimize: false,
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
