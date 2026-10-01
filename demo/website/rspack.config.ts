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

/** Build the tur WASM (boa + vello + swc) and copy the pkg assets to dist. */
class WasmBuildPlugin implements RspackPluginInstance {
    apply(compiler: Compiler): void {
        const buildWasm = () => {
            compiler
                .getInfrastructureLogger("WasmBuildPlugin")
                .info(
                    "Building WASM (multi-threaded, +atomics, --no-opt) with wasm-pack...",
                );
            // Multi-threaded engine workers (Web Workers via the
            // SharedArrayBuffer). The `+atomics` rustflags in
            // `.cargo/config.toml` apply to ALL wasm32 builds regardless of
            // profile, so workers spawn fine even with `--no-opt`.
            //
            // Using `--no-opt` instead of `--profile wasm-dev`:
            // - Skips `wasm-opt` post-processing (which interacts badly
            //   with boa_engine's AST codegen and traps at "null reference
            //   produced" during JS module evaluation).
            // - Uses default `panic = "unwind"` instead of `panic = "abort"`
            //   (the latter is required only for shared-memory linkage in
            //   some toolchain versions; current nightly works with unwind
            //   for our case).
            //
            // The engine is fully async — main thread drives `pump` via
            // `wasm_bindgen_futures::spawn_local`, worker blocks freely
            // inside `futures::executor::block_on(worker_loop)`.
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
                // Emit per-snippet files (e.g. the engine's web worker
                // helper, wasm-streams inline modules) preserving the
                // `snippets/<crate-hash>/<file>` path the JS glue expects.
                // Recurses into subdirs (the engine worker script lives
                // under `snippets/<crate-hash>/src/...`).
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
                                    new compiler.webpack.sources.RawSource(
                                        content,
                                    ),
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

export default defineConfig({
    optimization: {
        minimize: false,
    },
    devServer: {
        hot: false,
        liveReload: false,
        port: 8080,
        host: "0.0.0.0",
        allowedHosts: "all",
        // Always set COOP/COEP — the wasm multi-threaded backend uses
        // SharedArrayBuffer + Web Workers (via the in-tree worker_spawn), which
        // requires `self.crossOriginIsolated`. Without these headers
        // `Worker.postMessage` fails with
        // `DataCloneError: SharedArrayBuffer transfer requires
        // self.crossOriginIsolated` at engine init.
        //
        // COEP value: `require-corp` (NOT `credentialless`). Firefox
        // (desktop + Android) never implemented `credentialless` —
        // Chromium-only. With `credentialless`, Firefox silently ignores
        // the header and `crossOriginIsolated` stays false, so SAB is
        // unavailable and the probe fails. `require-corp` is universally
        // supported; the dev server only serves same-origin assets
        // (wasm, JS, snippets), so no cross-origin resource needs a
        // CORP opt-in.
        headers: {
            "Cross-Origin-Opener-Policy": "same-origin",
            "Cross-Origin-Embedder-Policy": "require-corp",
            "Cross-Origin-Resource-Policy": "same-origin",
            "Cache-Control": "no-store",
        },
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
        new rspack.CopyRspackPlugin({ patterns: [{ from: "public" }] }),
    ],
});
