import { defineConfig } from "@rspack/cli";

export default defineConfig({
  entry: {
    index: "./src/index.ts",
  },
  output: {
    filename: "[name].js",
    library: {
      type: "module",
    },
    clean: true,
  },
  experiments: {
    outputModule: true,
  },
  optimization: {
    usedExports: false,
    minimize: false,
  },
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
              target: "esnext",
            },
          },
        },
        type: "javascript/auto",
      },
    ],
  },
  resolve: {
    extensions: [".ts", ".js"],
  },
});
