import { defineConfig } from "@rspack/cli";
import { TurRspackPlugin } from "@tur/rspack-plugin";
import selfsigned from "selfsigned";

const { cert, private: key } = selfsigned.generate(
  [{ name: "commonName", value: "local.hpp2334.com" }],
  { days: 365, algorithm: "sha256" },
);

export default defineConfig({
  optimization: {
    minimize: false,
  },
  devServer: {
    hot: false,
    liveReload: false,
    client: false,
    host: "0.0.0.0",
    allowedHosts: "all",
    server: {
      type: "https",
      options: { cert, key },
    },
  },
  entry: {
    bundle: "./src/index.tsx",
  },
  output: {
    filename: "bundle.bin",
    publicPath: "",
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
  plugins: [
    new TurRspackPlugin({
      jsEntry: "bundle.bin",
    }),
  ],
});
