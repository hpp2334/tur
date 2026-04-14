#!/usr/bin/env node

const { Command } = require("commander");
const { execSync } = require("child_process");
const fs = require("fs/promises");
const fsSync = require("fs");
const http = require("http");
const path = require("path");
const Koa = require("koa");
const serve = require("koa-static");
const { WebSocketServer } = require("ws");

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

    const ws = new WebSocket("ws://" + location.host + "/__ws");
    ws.addEventListener("message", (e) => {
      if (e.data === "reload") location.reload();
    });
  </script>
</body>
</html>
`;
}

async function startServer(dir, port) {
  const app = new Koa();
  app.use(serve(dir));
  const server = http.createServer(app.callback());

  const wss = new WebSocketServer({ server, path: "/__ws" });

  function broadcast(msg) {
    for (const ws of wss.clients) {
      if (ws.readyState === 1) ws.send(msg);
    }
  }

  function close() {
    return new Promise((resolve) => {
      for (const ws of wss.clients) {
        ws.close();
      }
      server.close(() => resolve());
    });
  }

  server.listen(port, () => {
    console.log(`Serving at http://localhost:${port}`);
  });

  return { broadcast, close };
}

function watchJsFile(jsPath, onReload) {
  let lastTrigger = 0;
  const watcher = fsSync.watch(jsPath, (eventType) => {
    if (eventType !== "change") return;
    const now = Date.now();
    if (now - lastTrigger < 300) return;
    lastTrigger = now;
    console.log(`[${new Date().toLocaleTimeString()}] ${path.basename(jsPath)} changed — reloading`);
    onReload();
  });
  console.log(`Watching ${jsPath}`);
  return watcher;
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

    const serveDir = path.dirname(jsPath);
    const jsFilename = path.basename(jsPath);
    const wasmFiles = await fs.readdir(pkgDir);
    for (const file of wasmFiles) {
      await fs.copyFile(path.join(pkgDir, file), path.join(serveDir, file));
    }
    await fs.writeFile(path.join(serveDir, "index.html"), generateHtml(jsFilename));

    console.log(`Copied wasm files to ${serveDir}`);
    const { broadcast, close } = await startServer(serveDir, parseInt(options.port, 10));
    const watcher = watchJsFile(jsPath, () => broadcast("reload"));

    async function shutdown() {
      console.log("\nShutting down...");
      watcher.close();
      await close();
      process.exit(0);
    }

    process.on("SIGTERM", shutdown);
    process.on("SIGINT", shutdown);
  });

program.parse();
