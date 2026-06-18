import { execSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@rspack/cli";
import type { Compiler, RspackPluginInstance } from "@rspack/core";
import * as rspack from "@rspack/core";

const __dirname = dirname(fileURLToPath(import.meta.url));
const workspaceRoot = resolve(__dirname, "../../..");
const wasmDir = join(workspaceRoot, "libs", "tur-wasm");
const wasmPkgDir = join(wasmDir, "pkg");
const implDir = join(__dirname, "../tur-demo-impl");

/** Build the tur WASM (boa + vello + swc) and copy the pkg assets to dist. */
class WasmBuildPlugin implements RspackPluginInstance {
    apply(compiler: Compiler): void {
        const buildWasm = () => {
            compiler
                .getInfrastructureLogger("WasmBuildPlugin")
                .info("Building WASM with wasm-pack...");
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
                for (const file of readdirSync(wasmPkgDir)) {
                    if (!/\.(js|wasm|d\.ts)$/.test(file)) continue;
                    const content = readFileSync(join(wasmPkgDir, file));
                    compilation.emitAsset(
                        file,
                        new compiler.webpack.sources.RawSource(content),
                    );
                    logger.info(`Copied WASM asset: ${file}`);
                }
            },
        );
    }
}

/** Build the self-hosted playground (tur-demo-impl) and emit impl.js. */
class ImplBundlePlugin implements RspackPluginInstance {
    apply(compiler: Compiler): void {
        const buildImpl = () => {
            compiler
                .getInfrastructureLogger("ImplBundlePlugin")
                .info("Building tur-demo-impl...");
            execSync("pnpm build", { cwd: implDir, stdio: "inherit" });
        };
        compiler.hooks.beforeRun.tapPromise("ImplBundlePlugin", async () =>
            buildImpl(),
        );
        compiler.hooks.watchRun.tapPromise("ImplBundlePlugin", async () =>
            buildImpl(),
        );
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
