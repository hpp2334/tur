#!/usr/bin/env node

const { Command } = require("commander");
const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const http = require("http");

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

function serveDir(dir, port) {
  const mimeTypes = {
    ".html": "text/html",
    ".js": "application/javascript",
    ".wasm": "application/wasm",
  };

  const server = http.createServer((req, res) => {
    let urlPath = req.url.split("?")[0];
    if (urlPath === "/") urlPath = "/index.html";

    const filePath = path.join(dir, urlPath);
    if (!filePath.startsWith(dir)) {
      res.writeHead(403);
      res.end("Forbidden");
      return;
    }

    fs.readFile(filePath, (err, data) => {
      if (err) {
        res.writeHead(404);
        res.end("Not Found");
        return;
      }
      const ext = path.extname(filePath);
      res.writeHead(200, { "Content-Type": mimeTypes[ext] || "application/octet-stream" });
      res.end(data);
    });
  });

  server.listen(port, () => {
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
  .action((jsFile, options) => {
    const jsPath = path.resolve(jsFile);
    if (!fs.existsSync(jsPath)) {
      console.error(`File not found: ${jsPath}`);
      process.exit(1);
    }

    if (options.build) {
      buildWasm();
    }

    const jsFilename = path.basename(jsPath);
    fs.copyFileSync(jsPath, path.join(pkgDir, jsFilename));
    fs.writeFileSync(path.join(pkgDir, "index.html"), generateHtml(jsFilename));

    console.log(`Copied ${jsFilename} to ${pkgDir}`);
    serveDir(pkgDir, parseInt(options.port, 10));
  });

program.parse();
