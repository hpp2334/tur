import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@rspack/cli";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Builds the playground view as a single ES-module bundle (`impl.js`) that the
// thin website host loads via `TurWebsiteApp.loadAndRunModule`. `tur:*` are
// external — resolved at run time by the engine's boa module loader — so the
// bundle keeps its `import` statements. The bundle contains the Shell UI +
// inlined case sources; it calls `render(Shell)` on evaluation.
//
// `TUR_PLATFORM=android` builds the Android variant. `tur-android` registers no
// `Http` backend (so `TurNetPlugin` skips `tur:net`) and no file-picker backend
// (so `tur:filepicker` is absent) — and rspack's resolver rejects the `tur:`
// scheme for anything that isn't external. compile.ts therefore sources the
// optional module namespaces (Net, FilePicker) from a scheme-free alias
// (`@tur-pg/optional-ns`); resolve.alias points it at the real external
// re-exports for web, or an in-bundle empty stub for Android — so the Android
// bundle has no `tur:net` / `tur:filepicker` imports at all.
const android = process.env.TUR_PLATFORM === "android";

export default defineConfig({
    entry: {
        impl: "./src/index.ts",
    },
    output: {
        path: resolve(__dirname, "dist"),
        filename: "impl.js",
        clean: true,
        library: { type: "module" },
    },
    experiments: { outputModule: true },
    externals: {
        "tur:std": "tur:std",
        "tur:animation": "tur:animation",
        "tur-ext/demo-helper": "tur-ext/demo-helper",
        "tur:clipboard": "tur:clipboard",
        "tur:net": "tur:net",
        "tur:filepicker": "tur:filepicker",
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
        alias: {
            "@tur-pg/optional-ns": resolve(
                __dirname,
                android
                    ? "src/cases/optional-ns.android.ts"
                    : "src/cases/optional-ns.web.ts",
            ),
        },
    },
});
