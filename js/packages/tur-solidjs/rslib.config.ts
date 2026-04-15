import { defineConfig } from "@rslib/core";
import type { RsbuildConfig } from "@rslib/core";

export default defineConfig({
  lib: [
    {
      bundle: true,
      dts: { bundle: true },
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
    rspack(config: RsbuildConfig) {
      config.module = config.module || {};
      config.module.rules = config.module.rules || [];
      config.module.rules.push({
        test: /\.tsx$/,
        use: {
          loader: "babel-loader",
          options: {
            presets: [
              [
                "@babel/preset-typescript",
                { isTSX: true, allExtensions: true },
              ],
              [
                "solid",
                { generate: "universal", moduleName: "@tur/solidjs-renderer" },
              ],
            ],
          },
        },
      });
      config.externals = config.externals || [];
      (config.externals as string[]).push("@tur/solidjs-renderer");
    },
  },
});
