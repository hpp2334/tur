import { defineConfig } from "@rspack/cli";
import { BannerPlugin } from "@rspack/core";

export default defineConfig({
  entry: {
    cli: "./src/cli.ts",
  },
  output: {
    filename: "cli.cjs",
    chunkLoading: false,
    wasmLoading: false,
    clean: true,
  },
  target: "node",
  module: {
    rules: [
      {
        test: /\.ts$/,
        exclude: /node_modules/,
        use: {
          loader: "builtin:swc-loader",
          options: {
            jsc: {
              parser: { syntax: "typescript" },
            },
          },
        },
      },
      {
        resourceQuery: /raw/,
        type: "asset/source",
      },
    ],
  },
  plugins: [new BannerPlugin({ banner: "#!/usr/bin/env node", raw: true })],
  resolve: {
    extensions: [".ts", ".js"],
  },
});
