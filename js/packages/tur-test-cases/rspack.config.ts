import { defineConfig } from "@rspack/cli";
import { globSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const casesDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "react-cases");
const entries: Record<string, string> = {};

for (const dir of globSync("*/index.tsx", { cwd: casesDir })) {
  const name = dir.split("/")[0];
  entries[name] = `./react-cases/${dir}`;
}

export default defineConfig({
  mode: "production",
  optimization: {
    minimize: false,
  },
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
});
