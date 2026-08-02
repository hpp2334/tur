import { execSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@rspack/cli";
import type { Compiler, RspackPluginInstance } from "@rspack/core";
import * as rspack from "@rspack/core";

const __dirname = dirname(fileURLToPath(import.meta.url));
const wasmDir = resolve(__dirname, "native");
const wasmPkgDir = join(wasmDir, "pkg");
const implDir = resolve(__dirname, "../playground-view");

/** Build the tur WASM (boa + vello + swc) and copy the pkg assets to dist. */
class WasmBuildPlugin implements RspackPluginInstance {
    apply(compiler: Compiler): void {
        const buildWasm = () => {
            compiler
                .getInfrastructureLogger("WasmBuildPlugin")
                .info("Building WASM (threaded, +atomics) with wasm-pack...");
            // wasm-bindgen-rayon infrastructure (Cargo dep + atomics
            // rustflags + nightly toolchain) is in place, but the runtime
            // call is deferred to the host (see WasmRuntime::create
            // docstring for why). Plain `--dev` profile (panic=unwind)
            // keeps the engine working; the `wasm-dev` profile
            // (panic=abort) is only needed when actually using threads.
            execSync("wasm-pack build --target web --no-opt", {
                cwd: wasmDir,
                stdio: "inherit",
            });
        };
        compiler.hooks.beforeRun.tapPromise("WasmBuildPlugin", async () =>
            buildWasm(),
        );
        compiler.hooks.watchRun.tapPromise("WasmBuildPlugin", async () =>
            buildWasm(),
        );
        compiler.hooks.emit.tapPromise(
            "WasmBuildPlugin",
            async (compilation) => {
                const logger = compilation.getLogger("WasmBuildPlugin");
                // Emit top-level files (tur_website.{js,wasm,d.ts}).
                for (const file of readdirSync(wasmPkgDir)) {
                    if (!/\.(js|wasm|d\.ts)$/.test(file)) continue;
                    const content = readFileSync(join(wasmPkgDir, file));
                    compilation.emitAsset(
                        file,
                        new compiler.webpack.sources.RawSource(content),
                    );
                    logger.info(`Copied WASM asset: ${file}`);
                }
                // Emit per-snippet files (e.g. wasm-bindgen-rayon worker
                // helper, wasm-streams inline modules) preserving the
                // `snippets/<crate-hash>/<file>` path the JS glue expects.
                // Recurses into subdirs (rayon's workerHelpers lives under
                // `snippets/<crate-hash>/src/workerHelpers.js`).
                const snippetsDir = join(wasmPkgDir, "snippets");
                if (existsSync(snippetsDir)) {
                    const walk = (dir: string, relPrefix: string) => {
                        for (const entry of readdirSync(dir)) {
                            const abs = join(dir, entry);
                            const rel = `${relPrefix}/${entry}`;
                            if (statSync(abs).isDirectory()) {
                                walk(abs, rel);
                            } else {
                                const content = readFileSync(abs);
                                compilation.emitAsset(
                                    rel,
                                    new compiler.webpack.sources.RawSource(content),
                                );
                                logger.info(`Copied WASM snippet: ${rel}`);
                            }
                        }
                    };
                    walk(snippetsDir, "snippets");
                }
            },
        );
    }
}

/** Build the playground-view bundle and emit impl.js. */
class ImplBundlePlugin implements RspackPluginInstance {
    /** Timestamp (ms) of the last successful `pnpm build` of playground-view. */
    private lastBuilt = 0;
    apply(compiler: Compiler): void {
        // playground-view is emitted as a pre-built asset (dist/impl.js), so
        // its source is NOT part of the website's module graph — rspack's
        // watcher won't see edits there unless we explicitly register the
        // directory as a context dependency (done in `afterCompile` below).
        // Without that, regenerating case sources (gen-cases →
        // src/cases/generated.ts) never reached the running dev server,
        // requiring a manual restart.
        const implSrcDir = join(implDir, "src");
        const generatedCases = join(implSrcDir, "cases", "generated.ts");

        const needsRebuild = (): boolean => {
            try {
                return statSync(generatedCases).mtimeMs > this.lastBuilt;
            } catch {
                return true;
            }
        };
        const buildImpl = () => {
            compiler
                .getInfrastructureLogger("ImplBundlePlugin")
                .info("Building playground-view...");
            execSync("pnpm build", { cwd: implDir, stdio: "inherit" });
            this.lastBuilt = Date.now();
        };

        compiler.hooks.beforeRun.tapPromise("ImplBundlePlugin", async () =>
            buildImpl(),
        );
        compiler.hooks.watchRun.tapPromise("ImplBundlePlugin", async () => {
            // `modifiedFiles` is the set of paths that triggered this watch
            // run. Only rebuild impl when something under playground-view/src
            // changed (e.g. a regenerated case manifest); otherwise skip so
            // unrelated website edits don't pay the `pnpm build` cost. Fall
            // back to an mtime check when modifiedFiles is unavailable.
            const changed = (
                compiler as unknown as { modifiedFiles?: Set<string> }
            ).modifiedFiles;
            const touched = changed
                ? [...changed].some((f) => f.startsWith(implSrcDir))
                : needsRebuild();
            if (touched) buildImpl();
        });
        compiler.hooks.afterCompile.tap("ImplBundlePlugin", (compilation) => {
            // Watch playground-view/src so edits there (incl. regenerated case
            // sources) trigger watchRun + an impl rebuild.
            compilation.contextDependencies.add(implSrcDir);
        });
        compiler.hooks.emit.tapPromise(
            "ImplBundlePlugin",
            async (compilation) => {
                const logger = compilation.getLogger("ImplBundlePlugin");
                const content = readFileSync(join(implDir, "dist", "impl.js"));
                compilation.emitAsset(
                    "impl.js",
                    new compiler.webpack.sources.RawSource(content),
                );
                logger.info("Emitted impl.js");
            },
        );
    }
}

export default defineConfig({
    optimization: {
        minimize: false,
    },
    devServer: {
        hot: false,
        liveReload: false,
        server: process.env.TUR_TUNNEL ? undefined : "https",
        port: 8080,
        host: "0.0.0.0",
        allowedHosts: "all",
        headers: process.env.TUR_TUNNEL
            ? {
                  "Cross-Origin-Opener-Policy": "same-origin",
                  "Cross-Origin-Embedder-Policy": "credentialless",
                  "Cache-Control": "no-store",
              }
            : {},
    },
    entry: {
        main: "./src/index.tsx",
    },
    output: {
        publicPath: "",
        clean: true,
        ...(process.env.TUR_TUNNEL
            ? { filename: "[name].[contenthash].js" }
            : {}),
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
                            parser: { syntax: "typescript", tsx: true },
                        },
                    },
                },
            },
        ],
    },
    resolve: {
        extensions: [".tsx", ".ts", ".js"],
    },
    plugins: [
        new rspack.HtmlRspackPlugin({ template: "./index.html" }),
        new WasmBuildPlugin(),
        new ImplBundlePlugin(),
        new rspack.CopyRspackPlugin({ patterns: [{ from: "public" }] }),
    ],
});
