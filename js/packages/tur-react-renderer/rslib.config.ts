import { defineConfig } from "@rslib/core";

export default defineConfig({
    lib: [
        {
            bundle: true,
            dts: { bundle: true, abortOnError: false },
            format: "esm",
        },
    ],
    source: {
        entry: {
            index: "./src/index.ts",
        },
    },
    output: {
        target: "web",
    },
    tools: {
        rspack(config) {
            config.externals = config.externals || [];
            (config.externals as string[]).push("react", "react-reconciler");
        },
    },
});
