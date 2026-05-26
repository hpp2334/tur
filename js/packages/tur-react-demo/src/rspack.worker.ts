import {
    BrowserHttpImportEsmPlugin,
    builtinMemFs,
    rspack,
} from "@rspack/browser";

interface InitMessage {
    type: "init";
    deps: { name: string; code: string }[];
}

interface BuildMessage {
    type: "build";
    source: string;
    caseName: string;
}

type WorkerMessage = InitMessage | BuildMessage;

interface BuildResult {
    type: "result";
    compiled: string;
    error?: string;
}

const WORKSPACE_PKGS = new Set(["@tur/react", "@tur/react-renderer"]);

self.onmessage = async (event: MessageEvent<WorkerMessage>) => {
    if (event.data.type === "init") {
        const { deps } = event.data;
        const files: Record<string, string> = {};
        for (const dep of deps) {
            files[`/node_modules/${dep.name}/dist/index.js`] = dep.code;
            files[`/node_modules/${dep.name}/package.json`] = JSON.stringify({
                name: dep.name,
                main: "./dist/index.js",
                type: "module",
            });
        }
        builtinMemFs.volume.fromJSON(files);
        self.postMessage({ type: "init-done" });
        return;
    }

    const { source } = event.data;
    try {
        builtinMemFs.volume.fromJSON({
            ...builtinMemFs.volume.toJSON(),
            "/src/index.tsx": source,
        });

        const result = await new Promise<string>((resolve, reject) => {
            rspack(
                {
                    entry: "/src/index.tsx",
                    output: {
                        filename: "bundle.js",
                        library: { type: "iife" },
                        path: "/dist",
                    },
                    module: {
                        rules: [
                            {
                                test: /\.tsx?$/,
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
                                                },
                                            },
                                        },
                                    },
                                },
                            },
                        ],
                    },
                    resolve: {
                        extensions: [".tsx", ".ts", ".js"],
                    },
                    optimization: {
                        minimize: false,
                    },
                    experiments: {
                        buildHttp: {
                            allowedUris: ["https://esm.sh"],
                        },
                    },
                    plugins: [
                        new BrowserHttpImportEsmPlugin({
                            domain: "https://esm.sh",
                            dependencyUrl(req) {
                                if (WORKSPACE_PKGS.has(req.request)) {
                                    return undefined;
                                }
                            },
                        }),
                    ],
                },
                (err, stats) => {
                    if (err) {
                        reject(err);
                        return;
                    }
                    if (stats?.hasErrors()) {
                        const errors = stats
                            .toJson()
                            .errors?.map((e) => e.message)
                            .join("\n");
                        reject(new Error(errors));
                        return;
                    }
                    try {
                        const output = builtinMemFs.volume.readFileSync(
                            "/dist/bundle.js",
                            "utf-8",
                        );
                        resolve(output as string);
                    } catch (e) {
                        reject(e);
                    }
                },
            );
        });

        self.postMessage({ type: "result", compiled: result } as BuildResult);
    } catch (e) {
        self.postMessage({
            type: "result",
            compiled: "",
            error: e instanceof Error ? e.message : String(e),
        } as BuildResult);
    }
};
