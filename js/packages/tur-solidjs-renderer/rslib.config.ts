import { defineConfig } from "@rslib/core";

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
      "jsx-runtime": "./src/jsx-runtime.ts",
    },
  },
  output: {
    target: "web",
  },
});
