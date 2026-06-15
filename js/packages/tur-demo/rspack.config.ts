import { execSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@rspack/cli";
import type { Compiler, RspackPluginInstance } from "@rspack/core";
import * as rspack from "@rspack/core";
import ReactRefreshPlugin from "@rspack/plugin-react-refresh";

const __dirname = dirname(fileURLToPath(import.meta.url));
const workspaceRoot = resolve(__dirname, "../../..");
const wasmDir = join(workspaceRoot, "libs", "tur-wasm");
const wasmPkgDir = join(wasmDir, "pkg");
const testCasesDir = join(__dirname, "../tur-test-cases");

function findCaseNames(): string[] {
    const casesRoot = join(testCasesDir, "edgy-cases");
    return readdirSync(casesRoot, { withFileTypes: true })
        .filter((d) => d.isDirectory())
        .map((d) => d.name);
}

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

        compiler.hooks.beforeRun.tapPromise("WasmBuildPlugin", async () => {
            buildWasm();
        });

        compiler.hooks.watchRun.tapPromise("WasmBuildPlugin", async () => {
            buildWasm();
        });

        compiler.hooks.emit.tapPromise(
            "WasmBuildPlugin",
            async (compilation) => {
                const logger = compilation.getLogger("WasmBuildPlugin");
                const files = readdirSync(wasmPkgDir);
                const wasmAssets = files.filter(
                    (f) =>
                        f.endsWith(".js") ||
                        f.endsWith(".wasm") ||
                        f.endsWith(".d.ts"),
                );

                for (const file of wasmAssets) {
                    const src = join(wasmPkgDir, file);
                    const content = readFileSync(src);
                    const source = new compiler.webpack.sources.RawSource(
                        content,
                    );
                    compilation.emitAsset(file, source);
                    logger.info(`Copied WASM asset: ${file}`);
                }
            },
        );
    }
}

class TestCasesPlugin implements RspackPluginInstance {
    apply(compiler: Compiler): void {
        compiler.hooks.beforeRun.tapPromise("TestCasesPlugin", async () => {
            compiler
                .getInfrastructureLogger("TestCasesPlugin")
                .info("Building test cases...");
            execSync("npx rspack build", {
                cwd: testCasesDir,
                stdio: "inherit",
            });
        });

        compiler.hooks.watchRun.tapPromise("TestCasesPlugin", async () => {
            compiler
                .getInfrastructureLogger("TestCasesPlugin")
                .info("Building test cases...");
            execSync("npx rspack build", {
                cwd: testCasesDir,
                stdio: "inherit",
            });
        });

        compiler.hooks.emit.tapPromise(
            "TestCasesPlugin",
            async (compilation) => {
                const logger = compilation.getLogger("TestCasesPlugin");
                const distDir = join(testCasesDir, "dist");
                const casesRoot = join(testCasesDir, "edgy-cases");
                const caseNames = findCaseNames();
                const manifest: Record<string, string[]> = {};

                for (const name of caseNames) {
                    const jsFile = `${name}.js`;
                    const src = join(distDir, jsFile);
                    try {
                        const content = readFileSync(src);
                        const source = new compiler.webpack.sources.RawSource(
                            content,
                        );
                        compilation.emitAsset(`cases/${jsFile}`, source);
                    } catch {
                        logger.warn(`Test case not built: ${jsFile}`);
                    }

                    const caseDir = join(casesRoot, name);
                    const filesForCase: string[] = [];
                    try {
                        const files = readdirSync(caseDir);
                        for (const file of files) {
                            if (!/\.(ts|tsx)$/.test(file)) continue;
                            const content = readFileSync(
                                join(caseDir, file),
                                "utf-8",
                            );
                            const source =
                                new compiler.webpack.sources.RawSource(content);
                            compilation.emitAsset(
                                `sources/${name}/${file}`,
                                source,
                            );
                            filesForCase.push(file);
                        }
                    } catch {
                        logger.warn(`Test case source not found: ${name}`);
                    }
                    manifest[name] = filesForCase.sort();
                }

                const manifestJson = JSON.stringify(manifest);
                compilation.emitAsset(
                    "cases-manifest.json",
                    new compiler.webpack.sources.RawSource(manifestJson),
                );

                logger.info(
                    `Copied ${caseNames.length} test case bundles + sources`,
                );
            },
        );
    }
}

class RuntimeBundlePlugin implements RspackPluginInstance {
    apply(compiler: Compiler): void {
        const buildRuntime = () => {
            compiler
                .getInfrastructureLogger("RuntimeBundlePlugin")
                .info("Building boa runtime bundle...");
            execSync("npx rspack build --config runtime.rspack.config.ts", {
                cwd: __dirname,
                stdio: "inherit",
            });
        };

        compiler.hooks.beforeRun.tapPromise("RuntimeBundlePlugin", async () =>
            buildRuntime(),
        );

        compiler.hooks.watchRun.tapPromise("RuntimeBundlePlugin", async () =>
            buildRuntime(),
        );

        compiler.hooks.emit.tapPromise(
            "RuntimeBundlePlugin",
            async (compilation) => {
                const logger = compilation.getLogger("RuntimeBundlePlugin");
                const src = join(__dirname, ".runtime-build", "runtime.js");
                const content = readFileSync(src);
                const source = new compiler.webpack.sources.RawSource(content);
                compilation.emitAsset("runtime.js", source);
                logger.info("Emitted runtime.js");
            },
        );
    }
}

export default defineConfig({
    experiments: {
        css: true,
    },
    optimization: {
        minimize: false,
    },
    devServer: {
        hot: true,
        liveReload: false,
        client: {
            overlay: {
                errors: true,
                warnings: false,
            },
            ...(process.env.TUR_TUNNEL
                ? { webSocketURL: "auto://0.0.0.0:0/ws" }
                : {}),
        },
        server: process.env.TUR_TUNNEL ? undefined : "https",
        port: 8080,
        host: "0.0.0.0",
        allowedHosts: "all",
        headers: process.env.TUR_TUNNEL
            ? {
                  "Cross-Origin-Opener-Policy": "same-origin",
                  "Cross-Origin-Embedder-Policy": "credentialless",
                  "Cache-Control": "no-store",
                  "CDN-Cache-Control": "no-store",
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
                            parser: {
                                syntax: "typescript",
                                tsx: true,
                            },
                            transform: {
                                react: {
                                    runtime: "automatic",
                                    refresh: true,
                                },
                            },
                        },
                    },
                },
            },
            {
                test: /\.css$/,
                type: "css",
            },
        ],
    },
    resolve: {
        extensions: [".tsx", ".ts", ".js"],
    },
    plugins: [
        new rspack.HtmlRspackPlugin({
            template: "./index.html",
        }),
        new WasmBuildPlugin(),
        new TestCasesPlugin(),
        new RuntimeBundlePlugin(),
        new rspack.CopyRspackPlugin({
            patterns: [{ from: "public" }],
        }),
        new ReactRefreshPlugin(),
    ],
});
