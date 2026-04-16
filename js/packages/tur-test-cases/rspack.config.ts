import { defineConfig } from "@rspack/cli";
import { globSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const casesDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "solidjs-cases");
const entries: Record<string, string> = {};

for (const dir of globSync("*/index.tsx", { cwd: casesDir })) {
  const name = dir.split("/")[0];
  entries[name] = `./solidjs-cases/${dir}`;
}

export default defineConfig({
  entry: entries,
  output: {
    filename: "[name].js",
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
});
