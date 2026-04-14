#!/usr/bin/env node

const { Command } = require("commander");
const { execSync } = require("child_process");
const fs = require("fs/promises");
const path = require("path");
const Koa = require("koa");
const serve = require("koa-static");

const root = path.join(__dirname, "../../../../");
const wasmDir = path.join(root, "libs/tur-wasm");
const pkgDir = path.join(wasmDir, "pkg");

function buildWasm() {
  console.log("> wasm-pack build --target web");
  execSync("wasm-pack build --target web", { cwd: wasmDir, stdio: "inherit" });
}

function generateHtml(jsFilename) {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>tur wasm demo</title>
</head>
<body>
  <script type="module">
    import init, { TurWasmApp } from "./tur_wasm.js";

    async function run() {
      await init();
      const app = TurWasmApp.create();
      const resp = await fetch("./${jsFilename}");
      const source = await resp.text();
      app.load_and_run_js(source);
    }

    run().catch((e) => console.error(e));
  </script>
</body>
</html>
`;
}

async function serveDir(dir, port) {
  const app = new Koa();
  app.use(serve(dir));
  app.listen(port, () => {
    console.log(`Serving at http://localhost:${port}`);
  });
}

const program = new Command();
program.name("tur-wasm-cli").description("CLI for building and serving tur wasm demos").version(
  "0.1.0",
);

program
  .command("build")
  .description("Build the tur-wasm package with wasm-pack")
  .action(() => {
    buildWasm();
  });

program
  .command("serve")
  .description("Build wasm and serve a JS demo app")
  .argument("<jsFile>", "Path to the JS bundle file to serve")
  .option("-p, --port <number>", "Port to serve on", "3000")
  .option("--no-build", "Skip wasm build step")
  .action(async (jsFile, options) => {
    const jsPath = path.resolve(jsFile);
    try {
      await fs.access(jsPath);
    } catch {
      console.error(`File not found: ${jsPath}`);
      process.exit(1);
    }

    if (options.build) {
      buildWasm();
    }

    const jsFilename = path.basename(jsPath);
    await fs.copyFile(jsPath, path.join(pkgDir, jsFilename));
    await fs.writeFile(path.join(pkgDir, "index.html"), generateHtml(jsFilename));

    console.log(`Copied ${jsFilename} to ${pkgDir}`);
    serveDir(pkgDir, parseInt(options.port, 10));
  });

program.parse();
