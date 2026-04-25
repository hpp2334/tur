import { defineConfig } from "@rspack/cli";
import { TurRspackPlugin } from "@tur/rspack-plugin";

export default defineConfig({
  entry: {
    bundle: "./src/index.tsx",
  },
  output: {
    filename: "bundle.js",
    library: {
      type: "iife",
    },
    clean: true,
  },
  module: {
    rules: [
      {
        test: /\.tsx?$/,
        exclude: /node_modules/,
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
                {
                  generate: "universal",
                  moduleName: "@tur/solidjs-renderer",
                },
              ],
            ],
          },
        },
      },
    ],
  },
  resolve: {
    extensions: [".tsx", ".ts", ".js"],
  },
  plugins: [
    new TurRspackPlugin({
      jsEntry: "bundle.js",
    }),
  ],
});
